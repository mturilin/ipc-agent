use cid::Cid;
use fvm_ipld_encoding::RawBytes;
// Copyright 2022-2023 Protocol Labs
// SPDX-License-Identifier: MIT

use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::MethodNum;
use ipc_gateway::checkpoint::BatchCrossMsgs;
use ipc_gateway::{BottomUpCheckpoint, CrossMsg, Status, StorableMsg};
use ipc_sdk::address::IPCAddress;
use ipc_sdk::subnet_id::SubnetID;
use primitives::TCid;
use serde::{Deserialize, Serialize};

use crate::lotus::message::deserialize::{
    deserialize_subnet_id_from_map, deserialize_token_amount_from_str,
};
use crate::lotus::message::serialize::{
    serialize_subnet_id_to_str, serialize_token_amount_to_atto,
};
use crate::lotus::message::CIDMap;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct IPCGetPrevCheckpointForChildResponse {
    #[serde(rename = "CID")]
    pub cid: Option<CIDMap>,
}

/// The state of a gateway actor. The struct omits all fields that are not relevant for the
/// execution of the IPC agent.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct IPCReadGatewayStateResponse {
    pub check_period: ChainEpoch,
    pub applied_topdown_nonce: u64,
}

/// The state of a subnet actor. The struct omits all fields that are not relevant for the
/// execution of the IPC agent.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct IPCReadSubnetActorStateResponse {
    pub check_period: ChainEpoch,
    pub validator_set: ValidatorSet,
    pub min_validators: u64,
}

/// SubnetInfo is an auxiliary struct that collects relevant information about the state of a subnet
///
/// Note that the serialization and deserialization casing are different. Reason because for deserialization,
/// it is from the fvm actor, which is `PascalCase`. When serialize, we are using rust's default casing
#[derive(Debug, Serialize, Deserialize)]
pub struct SubnetInfo {
    /// Id of the subnet.
    #[serde(rename(deserialize = "ID"))]
    #[serde(deserialize_with = "deserialize_subnet_id_from_map")]
    #[serde(serialize_with = "serialize_subnet_id_to_str")]
    pub id: SubnetID,
    /// Collateral staked in the subnet.
    #[serde(rename(deserialize = "Stake"))]
    #[serde(deserialize_with = "deserialize_token_amount_from_str")]
    #[serde(serialize_with = "serialize_token_amount_to_atto")]
    pub stake: TokenAmount,
    /// Circulating supply available in the subnet.
    #[serde(rename(deserialize = "CircSupply"))]
    #[serde(deserialize_with = "deserialize_token_amount_from_str")]
    #[serde(serialize_with = "serialize_token_amount_to_atto")]
    pub circ_supply: TokenAmount,
    /// State of the Subnet (Initialized, Active, Killed)
    #[serde(rename(deserialize = "Status"))]
    pub status: Status,
}

/// We need to redefine the struct here due to:
/// In the actor, it is `Deserialize_tuple`, but when returned from json rpc endpoints, it's
/// actually `json` struct. The deserialization is not working because the agent is interpreting
/// the tuple as json.
#[derive(Deserialize, Serialize, Debug)]
pub struct ValidatorSet {
    pub validators: Option<Vec<Validator>>,
    // sequence number that uniquely identifies a validator set
    pub configuration_number: u64,
}

/// The validator struct. See `ValidatorSet` comment on why we need this duplicated definition.
#[derive(Deserialize, Serialize, Debug)]
pub struct Validator {
    pub addr: String,
    pub net_addr: String,
    pub weight: String,
}

/// This deserializes from the `gateway::Checkpoint`, we need to redefine
/// here because the Lotus API json serializes and the cbor tuple deserializer is not
/// able to pick it up automatically
#[derive(Deserialize, Serialize, Debug)]
pub struct BottomUpCheckpointResponse {
    #[serde(rename(deserialize = "Data"))]
    pub data: CheckpointData,
    #[serde(rename(deserialize = "Sig"))]
    #[serde(with = "serde_bytes")]
    pub sig: Option<Vec<u8>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CheckpointData {
    #[serde(rename(deserialize = "Source"))]
    #[serde(deserialize_with = "deserialize_subnet_id_from_map")]
    pub source: SubnetID,
    #[serde(rename(deserialize = "Proof"))]
    #[serde(with = "serde_bytes")]
    pub proof: Option<Vec<u8>>,
    #[serde(rename(deserialize = "Epoch"))]
    pub epoch: i64,
    #[serde(rename(deserialize = "Children"))]
    pub children: Option<Vec<CheckData>>,
    #[serde(rename(deserialize = "PrevCheck"))]
    pub prev_check: Option<CIDMap>,
    #[serde(rename(deserialize = "CrossMsgs"))]
    pub cross_msgs: Option<BatchCrossMsgWrapper>,
}

#[derive(PartialEq, Eq, Clone, Debug, Default, Deserialize, Serialize)]
pub struct BatchCrossMsgWrapper {
    #[serde(rename(deserialize = "CrossMsgs"))]
    pub cross_msgs: Option<Vec<CrossMsgsWrapper>>,
    #[serde(rename(deserialize = "Fee"))]
    #[serde(deserialize_with = "deserialize_token_amount_from_str")]
    pub fee: TokenAmount,
}

impl From<BatchCrossMsgWrapper> for BatchCrossMsgs {
    fn from(wrapper: BatchCrossMsgWrapper) -> Self {
        let cross_msgs = wrapper.cross_msgs.map(|cross_msgs| {
            cross_msgs
                .into_iter()
                .map(|cross_wrapper| CrossMsg {
                    msg: StorableMsg {
                        from: cross_wrapper.msg.from,
                        to: cross_wrapper.msg.to,
                        method: cross_wrapper.msg.method,
                        params: cross_wrapper.msg.params,
                        value: cross_wrapper.msg.value,
                        nonce: cross_wrapper.msg.nonce,
                    },
                    wrapped: cross_wrapper.wrapped,
                })
                .collect::<Vec<_>>()
        });

        BatchCrossMsgs {
            cross_msgs,
            fee: wrapper.fee,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CrossMsgsWrapper {
    pub msg: StorableMsgsWrapper,
    pub wrapped: bool,
}

#[derive(PartialEq, Eq, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct StorableMsgsWrapper {
    // TODO: @will,IPCAddress is currently serialized by default as a tuple,
    // we need to implement its map counterpart so it can be deserialized
    // using a map from Lotus.
    pub from: IPCAddress,
    pub to: IPCAddress,
    pub method: MethodNum,
    pub params: RawBytes,
    pub value: TokenAmount,
    pub nonce: u64,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CheckData {
    #[serde(rename(deserialize = "Source"))]
    #[serde(deserialize_with = "deserialize_subnet_id_from_map")]
    pub source: SubnetID,
    #[serde(rename(deserialize = "Checks"))]
    pub checks: Vec<CIDMap>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Votes {
    pub validators: Vec<Address>,
}

impl TryFrom<BottomUpCheckpointResponse> for BottomUpCheckpoint {
    type Error = anyhow::Error;

    fn try_from(checkpoint_response: BottomUpCheckpointResponse) -> Result<Self, Self::Error> {
        let prev_check = if let Some(prev_check) = checkpoint_response.data.prev_check {
            TCid::from(Cid::try_from(prev_check)?)
        } else {
            TCid::default()
        };
        log::debug!("previous checkpoint: {prev_check:?}");

        let children = if let Some(children) = checkpoint_response.data.children {
            children
                .into_iter()
                .map::<Result<_, Self::Error>, _>(|c| {
                    let checks: Result<Vec<_>, _> = c
                        .checks
                        .into_iter()
                        .map(|cid_map| Cid::try_from(cid_map).map(TCid::from))
                        .collect();

                    Ok(ipc_gateway::checkpoint::ChildCheck {
                        source: c.source,
                        checks: checks?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![]
        };
        log::debug!("children: {children:?}");

        let cross_msgs = if let Some(cross_msgs) = checkpoint_response.data.cross_msgs {
            BatchCrossMsgs::from(cross_msgs)
        } else {
            BatchCrossMsgs::default()
        };
        log::debug!("cross_msgs: {cross_msgs:?}");

        let data = ipc_gateway::checkpoint::CheckData {
            source: checkpoint_response.data.source,
            proof: checkpoint_response.data.proof.unwrap_or_default(),
            epoch: checkpoint_response.data.epoch,
            prev_check,
            children,
            cross_msgs,
        };
        Ok(BottomUpCheckpoint {
            data,
            sig: checkpoint_response.sig.unwrap_or_default(),
        })
    }
}

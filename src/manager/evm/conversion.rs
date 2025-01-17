// Copyright 2022-2023 Protocol Labs
// SPDX-License-Identifier: MIT
//! Type conversion between evm and fvm

use crate::checkpoint::{NativeBottomUpCheckpoint, NativeChildCheck};
use crate::manager::evm::manager::agent_subnet_to_evm_addresses;
use crate::manager::SubnetInfo;
use anyhow::anyhow;
use ethers::abi::{ParamType, Token};
use ethers::types::U256;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::{Address, Payload};
use fvm_shared::bigint::BigInt;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::MethodNum;
use ipc_gateway::checkpoint::{BatchCrossMsgs, CheckData, ChildCheck};
use ipc_gateway::{BottomUpCheckpoint, CrossMsg, Status, StorableMsg};
use ipc_sdk::address::IPCAddress;
use ipc_sdk::subnet_id::SubnetID;
use primitives::EthAddress;
use std::str::FromStr;

impl TryFrom<NativeChildCheck> for crate::manager::evm::subnet_contract::ChildCheck {
    type Error = anyhow::Error;

    fn try_from(value: NativeChildCheck) -> Result<Self, Self::Error> {
        let vec_to_array = |v: Vec<u8>| {
            let bytes = if v.len() > 32 {
                log::warn!("child check more than 32 bytes, taking only first 32 bytes");
                &v[0..32]
            } else {
                &v
            };

            let mut array = [0u8; 32];
            array.copy_from_slice(bytes);

            array
        };
        let checks: Vec<[u8; 32]> = value
            .checks
            .into_iter()
            .map(vec_to_array)
            .collect::<Vec<_>>();
        Ok(Self {
            source: crate::manager::evm::subnet_contract::SubnetID::try_from(&value.source)?,
            checks,
        })
    }
}

impl TryFrom<crate::manager::evm::subnet_contract::ChildCheck> for NativeChildCheck {
    type Error = anyhow::Error;

    fn try_from(
        value: crate::manager::evm::subnet_contract::ChildCheck,
    ) -> Result<Self, Self::Error> {
        let checks = value.checks.into_iter().map(|v| v.to_vec()).collect();
        Ok(Self {
            source: SubnetID::try_from(value.source)?,
            checks,
        })
    }
}

impl TryFrom<NativeBottomUpCheckpoint>
    for crate::manager::evm::subnet_contract::BottomUpCheckpoint
{
    type Error = anyhow::Error;

    fn try_from(value: NativeBottomUpCheckpoint) -> Result<Self, Self::Error> {
        let cross_msgs = value
            .cross_msgs
            .cross_msgs
            .unwrap_or_default()
            .into_iter()
            .map(|i| {
                crate::manager::evm::subnet_contract::CrossMsg::try_from(i)
                    .map_err(|e| anyhow!("cannot convert cross msg due to: {e:}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let children = value
            .children
            .into_iter()
            .map(|i| {
                crate::manager::evm::subnet_contract::ChildCheck::try_from(i)
                    .map_err(|e| anyhow!("cannot convert child check due to: {e:}"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut prev_hash = [0u8; 32];
        if let Some(v) = &value.prev_check {
            prev_hash.copy_from_slice(v);
        }

        let proof = if let Some(v) = value.proof {
            ethers::core::types::Bytes::from(v)
        } else {
            ethers::core::types::Bytes::default()
        };

        let b = crate::manager::evm::subnet_contract::BottomUpCheckpoint {
            source: crate::manager::evm::subnet_contract::SubnetID::try_from(&value.source)?,
            epoch: value.epoch as u64,
            fee: U256::from_str(&value.cross_msgs.fee.atto().to_string())?,
            cross_msgs,
            children,
            prev_hash,
            proof,
        };
        Ok(b)
    }
}

impl TryFrom<crate::manager::evm::subnet_contract::BottomUpCheckpoint>
    for NativeBottomUpCheckpoint
{
    type Error = anyhow::Error;

    fn try_from(
        value: crate::manager::evm::subnet_contract::BottomUpCheckpoint,
    ) -> Result<Self, Self::Error> {
        let children = value
            .children
            .into_iter()
            .map(NativeChildCheck::try_from)
            .collect::<anyhow::Result<_>>()?;

        let cross_msgs = value
            .cross_msgs
            .into_iter()
            .map(|i| {
                CrossMsg::try_from(i).map_err(|e| anyhow!("cannot convert cross msg due to: {e:}"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let b = NativeBottomUpCheckpoint {
            source: SubnetID::try_from(value.source)?,
            proof: Some(value.proof.to_vec()),
            epoch: value.epoch as ChainEpoch,
            prev_check: Some(value.prev_hash.to_vec()),
            children,
            cross_msgs: BatchCrossMsgs {
                cross_msgs: Some(cross_msgs),
                fee: TokenAmount::from_atto(value.fee.as_u128()),
            },
            sig: vec![],
        };
        Ok(b)
    }
}

impl TryFrom<BottomUpCheckpoint> for crate::manager::evm::subnet_contract::BottomUpCheckpoint {
    type Error = anyhow::Error;

    fn try_from(checkpoint: BottomUpCheckpoint) -> Result<Self, Self::Error> {
        // sig field of checkpoint not currently used for checkpoint verification
        let check_data = checkpoint.data;
        crate::manager::evm::subnet_contract::BottomUpCheckpoint::try_from(check_data)
    }
}

impl TryFrom<CheckData> for crate::manager::evm::subnet_contract::BottomUpCheckpoint {
    type Error = anyhow::Error;

    fn try_from(check_data: CheckData) -> Result<Self, Self::Error> {
        let cross_msgs = check_data
            .cross_msgs
            .cross_msgs
            .unwrap_or_default()
            .into_iter()
            .map(|i| {
                crate::manager::evm::subnet_contract::CrossMsg::try_from(i)
                    .map_err(|e| anyhow!("cannot convert cross msg due to: {e:}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let children = check_data
            .children
            .into_iter()
            .map(|i| {
                crate::manager::evm::subnet_contract::ChildCheck::try_from(i)
                    .map_err(|e| anyhow!("cannot convert child check due to: {e:}"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let b = crate::manager::evm::subnet_contract::BottomUpCheckpoint {
            source: crate::manager::evm::subnet_contract::SubnetID::try_from(&check_data.source)?,
            epoch: check_data.epoch as u64,
            fee: fil_to_eth_amount(&check_data.cross_msgs.fee)?,
            cross_msgs,
            children,

            // update these two parameters from caller
            prev_hash: [0; 32],
            proof: ethers::core::types::Bytes::default(),
        };
        Ok(b)
    }
}

impl TryFrom<CrossMsg> for crate::manager::evm::subnet_contract::CrossMsg {
    type Error = anyhow::Error;

    fn try_from(value: CrossMsg) -> Result<Self, Self::Error> {
        let c = crate::manager::evm::subnet_contract::CrossMsg {
            wrapped: value.wrapped,
            message: crate::manager::evm::subnet_contract::StorableMsg::try_from(value.msg)
                .map_err(|e| anyhow!("cannot convert storable msg due to: {e:}"))?,
        };
        Ok(c)
    }
}

impl TryFrom<IPCAddress> for crate::manager::evm::subnet_contract::Ipcaddress {
    type Error = anyhow::Error;

    fn try_from(value: IPCAddress) -> Result<Self, Self::Error> {
        Ok(crate::manager::evm::subnet_contract::Ipcaddress {
            subnet_id: crate::manager::evm::subnet_contract::SubnetID::try_from(&value.subnet()?)?,
            raw_address: crate::manager::evm::subnet_contract::FvmAddress::try_from(
                value.raw_addr()?,
            )?,
        })
    }
}

impl TryFrom<StorableMsg> for crate::manager::evm::subnet_contract::StorableMsg {
    type Error = anyhow::Error;

    fn try_from(value: StorableMsg) -> Result<Self, Self::Error> {
        let msg_value = fil_to_eth_amount(&value.value)?;

        log::info!(
            "storable message token amount: {:}, converted: {:?}",
            value.value.atto().to_string(),
            msg_value
        );

        let c = crate::manager::evm::subnet_contract::StorableMsg {
            from: crate::manager::evm::subnet_contract::Ipcaddress::try_from(value.from)
                .map_err(|e| anyhow!("cannot convert `from` ipc address msg due to: {e:}"))?,
            to: crate::manager::evm::subnet_contract::Ipcaddress::try_from(value.to)
                .map_err(|e| anyhow!("cannot convert `to`` ipc address due to: {e:}"))?,
            value: msg_value,
            nonce: value.nonce,
            // FIXME: we might a better way to handle the encoding of methods and params according to the type of message the cross-net message is targetting.
            method: (value.method as u32).to_be_bytes(),
            params: ethers::core::types::Bytes::from(value.params.to_vec()),
        };
        Ok(c)
    }
}

impl TryFrom<ChildCheck> for crate::manager::evm::subnet_contract::ChildCheck {
    type Error = anyhow::Error;

    fn try_from(value: ChildCheck) -> Result<Self, Self::Error> {
        let c = crate::manager::evm::subnet_contract::ChildCheck {
            source: crate::manager::evm::subnet_contract::SubnetID::try_from(&value.source)
                .map_err(|e| anyhow!("cannot convert subnet id due to: {e:}"))?,
            checks: value
                .checks
                .iter()
                .map(|c| {
                    let mut v = [0; 32];
                    // TODO: we should update the solidity contract to use bytes
                    v.copy_from_slice(&c.cid().to_bytes()[0..32]);
                    v
                })
                .collect(),
        };
        Ok(c)
    }
}

impl TryFrom<&SubnetID> for crate::manager::evm::subnet_contract::SubnetID {
    type Error = anyhow::Error;

    fn try_from(subnet: &SubnetID) -> Result<Self, Self::Error> {
        Ok(crate::manager::evm::subnet_contract::SubnetID {
            root: subnet.root_id(),
            route: agent_subnet_to_evm_addresses(subnet)?,
        })
    }
}

impl TryFrom<crate::manager::evm::subnet_contract::FvmAddress> for Address {
    type Error = anyhow::Error;

    fn try_from(
        value: crate::manager::evm::subnet_contract::FvmAddress,
    ) -> Result<Self, Self::Error> {
        let protocol = value.addr_type;
        let addr = bytes_to_fvm_addr(protocol, &value.payload)?;
        Ok(addr)
    }
}

/// It takes the bytes from an FVMAddress represented in Solidity and
/// converts it into the corresponding FVM address Rust type.
fn bytes_to_fvm_addr(protocol: u8, bytes: &[u8]) -> anyhow::Result<Address> {
    let addr = match protocol {
        1 => Address::from_bytes(&[[1u8].as_slice(), bytes].concat())?,
        4 => {
            let mut data = ethers::abi::decode(
                &[ParamType::Tuple(vec![
                    ParamType::Uint(32),
                    ParamType::Uint(32),
                    ParamType::Bytes,
                ])],
                bytes,
            )?;

            let mut data = data
                .pop()
                .ok_or_else(|| anyhow!("invalid tuple data length"))?
                .into_tuple()
                .ok_or_else(|| anyhow!("not tuple"))?;

            let raw_bytes = data
                .pop()
                .ok_or_else(|| anyhow!("invalid length, should be 3"))?
                .into_bytes()
                .ok_or_else(|| anyhow!("invalid bytes"))?;
            let len = data
                .pop()
                .ok_or_else(|| anyhow!("invalid length, should be 3"))?
                .into_uint()
                .ok_or_else(|| anyhow!("invalid uint"))?
                .as_u128();
            let namespace = data
                .pop()
                .ok_or_else(|| anyhow!("invalid length, should be 3"))?
                .into_uint()
                .ok_or_else(|| anyhow!("invalid uint"))?
                .as_u64();

            if len as usize != raw_bytes.len() {
                return Err(anyhow!("bytes len not match"));
            }
            Address::new_delegated(namespace, &raw_bytes)?
        }
        _ => return Err(anyhow!("address not support now")),
    };
    Ok(addr)
}

impl TryFrom<crate::manager::evm::gateway::Subnet> for SubnetInfo {
    type Error = anyhow::Error;

    fn try_from(value: crate::manager::evm::gateway::Subnet) -> Result<Self, Self::Error> {
        Ok(SubnetInfo {
            id: SubnetID::try_from(value.id)?,
            stake: eth_to_fil_amount(&value.stake)?,
            circ_supply: eth_to_fil_amount(&value.circ_supply)?,
            status: match value.status {
                1 => Status::Active,
                2 => Status::Inactive,
                3 => Status::Killed,
                _ => return Err(anyhow!("invalid status: {:}", value.status)),
            },
        })
    }
}

impl TryFrom<crate::manager::evm::gateway::FvmAddress> for Address {
    type Error = anyhow::Error;

    fn try_from(value: crate::manager::evm::gateway::FvmAddress) -> Result<Self, Self::Error> {
        let protocol = value.addr_type;
        let addr = bytes_to_fvm_addr(protocol, &value.payload)?;
        Ok(addr)
    }
}

impl From<Address> for crate::manager::evm::subnet_contract::FvmAddress {
    fn from(value: Address) -> Self {
        crate::manager::evm::subnet_contract::FvmAddress {
            addr_type: value.protocol() as u8,
            payload: addr_payload_to_bytes(value.into_payload()),
        }
    }
}

impl TryFrom<crate::manager::evm::gateway::SubnetID> for SubnetID {
    type Error = anyhow::Error;

    fn try_from(value: crate::manager::evm::gateway::SubnetID) -> Result<Self, Self::Error> {
        let children = value
            .route
            .iter()
            .map(ethers_address_to_fil_address)
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(SubnetID::new(value.root, children))
    }
}

impl TryFrom<crate::manager::evm::subnet_contract::SubnetID> for SubnetID {
    type Error = anyhow::Error;

    fn try_from(
        value: crate::manager::evm::subnet_contract::SubnetID,
    ) -> Result<Self, Self::Error> {
        let children = value
            .route
            .iter()
            .map(ethers_address_to_fil_address)
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(SubnetID::new(value.root, children))
    }
}

/// Converts a Rust type FVM address into its underlying payload
/// so it can be represented internally in a Solidity contract.
fn addr_payload_to_bytes(payload: Payload) -> ethers::types::Bytes {
    match payload {
        Payload::Secp256k1(v) => ethers::types::Bytes::from(v),
        Payload::Delegated(d) => {
            let addr = d.subaddress();
            let b = ethers::abi::encode(&[Token::Tuple(vec![
                Token::Uint(U256::from(d.namespace())),
                Token::Uint(U256::from(addr.len())),
                Token::Bytes(addr.to_vec()),
            ])]);
            ethers::types::Bytes::from(b)
        }
        _ => unimplemented!(),
    }
}

impl TryFrom<Address> for crate::manager::evm::gateway::FvmAddress {
    type Error = anyhow::Error;

    fn try_from(subnet: Address) -> Result<Self, Self::Error> {
        Ok(crate::manager::evm::gateway::FvmAddress {
            addr_type: subnet.protocol() as u8,
            payload: addr_payload_to_bytes(subnet.into_payload()),
        })
    }
}

impl TryFrom<&SubnetID> for crate::manager::evm::gateway::SubnetID {
    type Error = anyhow::Error;

    fn try_from(subnet: &SubnetID) -> Result<Self, Self::Error> {
        Ok(crate::manager::evm::gateway::SubnetID {
            root: subnet.root_id(),
            route: agent_subnet_to_evm_addresses(subnet)?,
        })
    }
}

impl TryFrom<crate::manager::evm::gateway::Ipcaddress> for IPCAddress {
    type Error = anyhow::Error;

    fn try_from(value: crate::manager::evm::gateway::Ipcaddress) -> Result<Self, Self::Error> {
        let addr = Address::try_from(value.raw_address)?;
        let i = IPCAddress::new(&SubnetID::try_from(value.subnet_id)?, &addr)?;
        Ok(i)
    }
}

impl TryFrom<crate::manager::evm::gateway::StorableMsg> for StorableMsg {
    type Error = anyhow::Error;

    fn try_from(value: crate::manager::evm::gateway::StorableMsg) -> Result<Self, Self::Error> {
        let s = StorableMsg {
            from: IPCAddress::try_from(value.from)?,
            to: IPCAddress::try_from(value.to)?,
            method: u32::from_be_bytes(value.method) as MethodNum,
            params: RawBytes::from(value.params.to_vec()),
            value: eth_to_fil_amount(&value.value)?,
            nonce: value.nonce,
        };
        Ok(s)
    }
}

impl TryFrom<crate::manager::evm::gateway::CrossMsg> for CrossMsg {
    type Error = anyhow::Error;

    fn try_from(value: crate::manager::evm::gateway::CrossMsg) -> Result<Self, Self::Error> {
        let c = CrossMsg {
            wrapped: value.wrapped,
            msg: StorableMsg::try_from(value.message)?,
        };
        Ok(c)
    }
}

impl TryFrom<crate::manager::evm::subnet_contract::Ipcaddress> for IPCAddress {
    type Error = anyhow::Error;

    fn try_from(
        value: crate::manager::evm::subnet_contract::Ipcaddress,
    ) -> Result<Self, Self::Error> {
        let addr = Address::try_from(value.raw_address)?;
        let i = IPCAddress::new(&SubnetID::try_from(value.subnet_id)?, &addr)?;
        Ok(i)
    }
}

impl TryFrom<crate::manager::evm::subnet_contract::StorableMsg> for StorableMsg {
    type Error = anyhow::Error;

    fn try_from(
        value: crate::manager::evm::subnet_contract::StorableMsg,
    ) -> Result<Self, Self::Error> {
        let s = StorableMsg {
            from: IPCAddress::try_from(value.from)?,
            to: IPCAddress::try_from(value.to)?,
            method: u32::from_be_bytes(value.method) as MethodNum,
            params: RawBytes::from(value.params.to_vec()),
            value: TokenAmount::from_atto(value.value.as_u128()),
            nonce: value.nonce,
        };
        Ok(s)
    }
}

impl TryFrom<crate::manager::evm::subnet_contract::CrossMsg> for CrossMsg {
    type Error = anyhow::Error;

    fn try_from(
        value: crate::manager::evm::subnet_contract::CrossMsg,
    ) -> Result<Self, Self::Error> {
        let c = CrossMsg {
            wrapped: value.wrapped,
            msg: StorableMsg::try_from(value.message)?,
        };
        Ok(c)
    }
}

/// Converts a Fil TokenAmount into an ethers::U256 amount.
pub fn fil_to_eth_amount(amount: &TokenAmount) -> anyhow::Result<U256> {
    let str = amount.atto().to_string();
    Ok(U256::from_dec_str(&str)?)
}

/// Converts an ethers::U256 TokenAmount into a FIL amount.
pub fn eth_to_fil_amount(amount: &U256) -> anyhow::Result<TokenAmount> {
    let v = BigInt::from_str(&amount.to_string())?;
    Ok(TokenAmount::from_atto(v))
}

pub fn ethers_address_to_fil_address(addr: &ethers::types::Address) -> anyhow::Result<Address> {
    let raw_addr = format!("{addr:?}");
    log::debug!("raw evm subnet addr: {raw_addr:}");

    let eth_addr = EthAddress::from_str(&raw_addr)?;
    Ok(Address::from(eth_addr))
}

#[cfg(test)]
mod tests {
    use crate::manager::evm::conversion::eth_to_fil_amount;
    use crate::manager::evm::subnet_contract::FvmAddress;
    use fvm_shared::{address::Address, bigint::BigInt, econ::TokenAmount};
    use primitives::EthAddress;
    use std::str::FromStr;

    use super::fil_to_eth_amount;

    #[test]
    fn test_fvm_address_encoding() {
        let test_evm_addr =
            EthAddress::from_str("0x1A79385eAd0e873FE0C441C034636D3Edf7014cC").unwrap();
        let addr = Address::from(test_evm_addr);

        let fvm_address = FvmAddress::try_from(addr).unwrap();
        assert_eq!(hex::encode(&fvm_address.payload), "0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000141a79385ead0e873fe0c441c034636d3edf7014cc000000000000000000000000");

        let address = Address::try_from(fvm_address).unwrap();

        assert_eq!(addr, address);
    }

    #[test]
    fn test_amount_conversion() {
        let v = BigInt::from_str("100000000000000").unwrap();
        let fil_amount = TokenAmount::from_atto(v);

        let eth_amount = fil_to_eth_amount(&fil_amount).unwrap();
        let test_amount = eth_to_fil_amount(&eth_amount).unwrap();
        assert_eq!(test_amount, fil_amount);
    }
}

// Copyright 2022-2023 Protocol Labs
// SPDX-License-Identifier: MIT
//! Bottom up checkpoint manager

use crate::checkpoint::{CheckpointManager, CheckpointMetadata, CheckpointQuery};
use crate::config::Subnet;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use ipc_gateway::checkpoint::BatchCrossMsgs;
use ipc_sdk::subnet_id::SubnetID;
use std::fmt::{Display, Formatter};

/// Native bottom up checkpoint struct independent of chain specific implementations.
/// The goal of this struct is to have a common checkpoint data structure that can be
/// eventually converted into their runtime-specific representations.
/// We need this type because some fields take different types in different runtime implementations,
/// such as `prev_check` is a cid in fvm but bytes in evm.
#[derive(Debug)]
pub struct NativeBottomUpCheckpoint {
    pub source: SubnetID,
    pub proof: Option<Vec<u8>>,
    pub epoch: ChainEpoch,
    pub prev_check: Option<Vec<u8>>,
    pub children: Vec<NativeChildCheck>,
    pub cross_msgs: BatchCrossMsgs,

    pub sig: Vec<u8>,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct NativeChildCheck {
    pub source: SubnetID,
    pub checks: Vec<Vec<u8>>,
}

/// The trait that handles the bottom up checkpoint submission data preparation and actual submission.
#[async_trait]
pub trait BottomUpHandler: Send + Sync + CheckpointQuery<NativeBottomUpCheckpoint> {
    /// Fetch the checkpoint template at the specified epoch
    async fn checkpoint_template(&self, epoch: ChainEpoch) -> Result<NativeBottomUpCheckpoint>;
    /// Populate previous checkpoint hash for the checkpoint
    async fn populate_prev_hash(
        &self,
        template: &mut NativeBottomUpCheckpoint,
        subnet: &SubnetID,
        previous_epoch: ChainEpoch,
    ) -> Result<()>;
    /// Populate the proof for the checkpoint
    async fn populate_proof(&self, template: &mut NativeBottomUpCheckpoint) -> Result<()>;
    /// Submit the checkpoint for validator
    async fn submit(
        &self,
        validator: &Address,
        checkpoint: NativeBottomUpCheckpoint,
    ) -> Result<ChainEpoch>;
}

pub struct BottomUpManager<P, C> {
    metadata: CheckpointMetadata,
    parent_handler: P,
    child_handler: C,
}

impl<P: BottomUpHandler, C: BottomUpHandler> BottomUpManager<P, C> {
    pub async fn new(
        parent: Subnet,
        child: Subnet,
        parent_handler: P,
        child_handler: C,
    ) -> Result<Self> {
        let period = parent_handler.checkpoint_period(&child.id).await?;
        Ok(Self {
            metadata: CheckpointMetadata {
                parent,
                child,
                period,
            },
            parent_handler,
            child_handler,
        })
    }
}

impl<P: BottomUpHandler, C: BottomUpHandler> Display for BottomUpManager<P, C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "bottom-up, parent: {:}, child: {:}",
            self.metadata.parent.id, self.metadata.child.id
        )
    }
}

#[async_trait]
impl<P: BottomUpHandler, C: BottomUpHandler> CheckpointManager for BottomUpManager<P, C> {
    /// Get the subnet config that this manager is submitting checkpoints to. For example, if it is
    /// top down checkpoints, target subnet return the child subnet config. If it is bottom up, target
    /// subnet returns parent subnet.
    fn target_subnet(&self) -> &Subnet {
        &self.metadata.parent
    }

    /// Getter for the parent subnet this checkpoint manager is handling
    fn parent_subnet(&self) -> &Subnet {
        &self.metadata.parent
    }

    /// Getter for the target subnet this checkpoint manager is handling
    fn child_subnet(&self) -> &Subnet {
        &self.metadata.child
    }

    /// The checkpoint period that the current manager is submitting upon
    fn checkpoint_period(&self) -> ChainEpoch {
        self.metadata.period
    }

    /// Get the list of validators that should submit checkpoints
    async fn validators(&self) -> Result<Vec<Address>> {
        self.parent_handler
            .validators(&self.metadata.child.id)
            .await
    }

    /// Obtain the last executed epoch of the checkpoint submission
    async fn last_executed_epoch(&self) -> Result<ChainEpoch> {
        self.parent_handler
            .last_executed_epoch(&self.metadata.child.id)
            .await
    }

    /// The current epoch in the subnet that the checkpoints should be submitted to
    async fn current_epoch(&self) -> Result<ChainEpoch> {
        self.child_handler.current_epoch().await
    }

    /// Submit the checkpoint based on the current epoch to submit and the previous epoch that was
    /// already submitted.
    async fn submit_checkpoint(&self, epoch: ChainEpoch, validator: &Address) -> Result<()> {
        let mut template = self.child_handler.checkpoint_template(epoch).await?;
        log::debug!("bottom up template: {template:?}");

        self.child_handler.populate_proof(&mut template).await?;
        log::debug!("bottom up checkpoint proof: {:?}", template.proof);

        let prev_epoch = epoch - self.metadata.period;
        self.parent_handler
            .populate_prev_hash(&mut template, &self.metadata.child.id, prev_epoch)
            .await?;
        log::debug!("bottom up checkpoint prev check: {:?}", template.prev_check);

        log::info!("bottom up checkpoint to submit: {template:?}");

        self.parent_handler
            .submit(validator, template)
            .await
            .map_err(|e| anyhow!("cannot submit bottom up checkpoint due to: {e:}"))?;

        Ok(())
    }

    /// Checks if the validator has already submitted in the epoch
    async fn should_submit_in_epoch(&self, validator: &Address, epoch: ChainEpoch) -> Result<bool> {
        let has_voted = self
            .parent_handler
            .has_voted(&self.metadata.child.id, epoch, validator)
            .await
            .map_err(|e| anyhow!("cannot check if validator: {validator:} has voted in epoch: {epoch:} due to: {e:}"))?;
        Ok(!has_voted)
    }

    /// Performs checks to see if the subnet is ready for checkpoint submission. If `true` means the
    /// subnet is ready for submission, else means the subnet is not ready. Bottom up default to true.
    async fn presubmission_check(&self) -> Result<bool> {
        Ok(true)
    }
}

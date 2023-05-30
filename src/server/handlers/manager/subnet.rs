// Copyright 2022-2023 Protocol Labs
// SPDX-License-Identifier: MIT
//! The shared subnet manager module for all subnet management related RPC method calls.

use crate::config::{ReloadableConfig, Subnet};
use crate::manager::{EthSubnetManager, LotusSubnetManager, SubnetManager};
use ipc_identity::Wallet;
use ipc_sdk::subnet_id::SubnetID;
use std::sync::{Arc, RwLock};

/// The subnet manager connection that holds the subnet config and the manager instance.
pub struct Connection {
    subnet: Subnet,
    manager: Box<dyn SubnetManager + 'static>,
}

impl Connection {
    /// Get the subnet config.
    pub fn subnet(&self) -> &Subnet {
        &self.subnet
    }

    /// Get the subnet manager instance.
    pub fn manager(&self) -> &Box<dyn SubnetManager> {
        &self.manager
    }
}

/// The json rpc subnet manager connection pool. This struct can be shared by all the subnet methods.
/// As such, there is no need to re-init the same SubnetManager for different methods to reuse connections.
pub struct SubnetManagerPool {
    config: Arc<ReloadableConfig>,
    wallet_store: Arc<RwLock<Wallet>>,
}

impl SubnetManagerPool {
    pub fn new(reload_config: Arc<ReloadableConfig>, wallet_store: Arc<RwLock<Wallet>>) -> Self {
        Self {
            config: reload_config,
            wallet_store,
        }
    }

    /// Get the connection instance for the subnet.
    pub fn get(&self, subnet: &SubnetID) -> Option<Connection> {
        let config = self.config.get_config();
        let subnets = &config.subnets;
        match subnets.get(subnet) {
            Some(subnet) => {
                if subnet.evm {
                    let manager = Box::new(EthSubnetManager::from_subnet(subnet).ok()?);
                    Some(Connection {
                        manager,
                        subnet: subnet.clone(),
                    })
                } else {
                    let manager = Box::new(LotusSubnetManager::from_subnet_with_wallet_store(
                        subnet,
                        self.wallet_store.clone(),
                    ));
                    Some(Connection {
                        manager,
                        subnet: subnet.clone(),
                    })
                }
            }
            None => None,
        }
    }
}

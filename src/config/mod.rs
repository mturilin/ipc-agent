// Copyright 2022-2023 Protocol Labs
// SPDX-License-Identifier: MIT
//! Provides a simple way of reading configuration files.
//!
//! Reads a TOML config file for the IPC Agent and deserializes it in a type-safe way into a
//! [`Config`] struct.

mod deserialize;
mod reload;
mod server;
pub mod subnet;

mod serialize;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use deserialize::deserialize_subnets_from_vec;
use ipc_sdk::subnet_id::SubnetID;
pub use reload::ReloadableConfig;
use serde::{Deserialize, Serialize};
use serialize::serialize_subnets_to_str;
pub use server::JSON_RPC_ENDPOINT;
pub use server::{json_rpc_methods, Server};
pub use subnet::Subnet;

pub const JSON_RPC_VERSION: &str = "2.0";

/// DefaulDEFAULT_CHAIN_IDSUBNET_e
pub const DEFAULT_CONFIG_TEMPLATE: &str = r#"
    [server]
    json_rpc_address = "127.0.0.1:3030"

    [[subnets]]
    id = "/r123"
    network_name = "test"

    [subnets.config]
    network_type = "fvm"
    gateway_addr = "f01"
    jsonrpc_api_http = "http://127.0.0.1:3030/rpc/v1"
    accounts = ["f01", "f01"]

    [[subnets]]
    id = "/r1234"
    network_name = "test2"

    [subnets.config]
    network_type = "fevm"
    provider_http = "http://127.0.0.1:3030/rpc/v1"
    registry_addr = "0x6be1ccf648c74800380d0520d797a170c808b624"
    gateway_addr = "0x6be1ccf648c74800380d0520d797a170c808b624"
    accounts = ["0x6be1ccf648c74800380d0520d797a170c808b624", "0x6be1ccf648c74800380d0520d797a170c808b624"]
"#;

/// The top-level struct representing the config. Calls to [`Config::from_file`] deserialize into
/// this struct.
#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct Config {
    pub server: Server,
    #[serde(deserialize_with = "deserialize_subnets_from_vec", default)]
    #[serde(serialize_with = "serialize_subnets_to_str")]
    pub subnets: HashMap<SubnetID, Subnet>,
}

impl Config {
    /// Reads a TOML configuration in the `s` string and returns a [`Config`] struct.
    pub fn from_toml_str(s: &str) -> Result<Self> {
        let config = toml::from_str(s)?;
        Ok(config)
    }

    /// Reads a TOML configuration file specified in the `path` and returns a [`Config`] struct.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: Config = Config::from_toml_str(contents.as_str())?;
        Ok(config)
    }

    /// Reads a TOML configuration file specified in the `path` and returns a [`Config`] struct.
    pub async fn from_file_async(path: impl AsRef<Path>) -> Result<Self> {
        let contents = tokio::fs::read_to_string(path).await?;
        Config::from_toml_str(contents.as_str())
    }

    pub async fn write_to_file_async(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = toml::to_string(self)?;
        tokio::fs::write(path, content.into_bytes()).await?;
        Ok(())
    }

    pub fn add_subnet(&mut self, subnet: Subnet) {
        self.subnets.insert(subnet.id.clone(), subnet);
    }

    pub fn remove_subnet(&mut self, subnet_id: &SubnetID) {
        self.subnets.remove(subnet_id);
    }
}

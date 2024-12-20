pub mod hyperlane;

pub use hyperlane::*;
use starknet::core::types::Felt;

use std::collections::HashMap;

use alloy::hex::FromHex;
use alloy::primitives::Address;
use url::Url;

use crate::configs::evm_config::{EvmChainName, EvmConfig};

#[derive(Debug, Default, Clone)]
pub struct HyperlaneValidatorsMapping(HashMap<EvmChainName, HashMap<Felt, u8>>);

impl HyperlaneValidatorsMapping {
    pub async fn from_config(config: &EvmConfig) -> anyhow::Result<Self> {
        let mut contracts = HashMap::new();

        for (chain_name, chain_config) in config.chains() {
            let rpc_url: Url = chain_config.rpc_url.parse()?;
            let address = Address::from_hex(&chain_config.hyperlane_address)
                .map_err(|e| anyhow::anyhow!("Invalid hyperlane address for {chain_name:?}: {e}"))?;
            let rpc_client = HyperlaneClient::new(rpc_url, address).await;

            let validators = rpc_client.get_validators_with_index().await?;
            contracts.insert(*chain_name, validators);
        }

        Ok(Self(contracts))
    }

    /// Get the available validators for a chain & their indexes
    pub fn get_validators(&self, chain_name: &EvmChainName) -> Option<&HashMap<Felt, u8>> {
        self.0.get(chain_name)
    }

    /// Get all configured chains names
    pub fn chain_names(&self) -> Vec<EvmChainName> {
        self.0.keys().cloned().collect()
    }

    /// Check if the provided chain is supported
    pub fn is_supported_chain(&self, chain: &EvmChainName) -> bool {
        self.0.contains_key(chain)
    }
}

use std::convert::TryInto;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct QkcAddress {
    pub coinbase: Vec<u8>,
    shard_id: Vec<u8>,
    chain_id: Vec<u8>
}

impl QkcAddress {
    pub fn coinbase(&self) -> String {
        format!("0x{}", hex::encode(&self.coinbase))
    }

    pub fn full_shard_key(&self) -> String {
        let shard_id = hex::encode(&self.shard_id);
        let chain_id = hex::encode(&self.chain_id);

        format!("0x{}{}", chain_id, shard_id)
    }

    pub fn chain_id(&self) -> u16 {
        u16::from_be_bytes(self.chain_id.clone().try_into().unwrap())
    }

    pub fn to_string(&self) -> String {
        let shard_id = hex::encode(&self.shard_id);
        let chain_id = hex::encode(&self.chain_id);
        let full_shard_chain = format!("{}{}", chain_id, shard_id);
        format!("{}{}", self.coinbase(), full_shard_chain)
    }

    pub fn new_from_coinbase(coinbase: &str) -> Result<Self> {
        Ok(Self {
            coinbase: hex::decode(&coinbase[2..])?,
            shard_id: vec![0],
            chain_id: vec![0]
        })
    }

    pub fn new(coinbase: &str, chain_id: u16, shard_id: u16) -> Result<QkcAddress> {
        Ok(Self {
            coinbase: hex::decode(&coinbase[2..])?,
            shard_id: shard_id.to_be_bytes().into(),
            chain_id: chain_id.to_be_bytes().into()
        })
    }

    pub fn new_full(address: &str) -> Result<QkcAddress> {
        Ok(
            Self {
                coinbase: hex::decode(&address[2..42])?,
                shard_id: hex::decode(&address[46..50])?,
                chain_id: hex::decode(&address[42..46])?
            }
        )
    }
}
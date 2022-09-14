use std::{vec, io::BufReader};

use serde::{Serialize, Deserialize};
use anyhow::Result;

use crate::types::QkcAddress;

#[derive(Debug, Clone)]
pub struct Qkc {
    pub(crate) client: reqwest::Client,
    pub(crate) url: String,
}

impl Qkc {
    pub async fn network_info(&self) -> Result<NetworkInfo> {
        let req = Request::<String>::new(Method::NetworkInfo, None);

        let res: RpcResponse<NetworkInfo> = self.client.post(&self.url).json(&req).send().await?.json().await?;
        
        Ok(res.result)
    }

    pub async fn get_transaction_count(&self, address: &QkcAddress) -> Result<u128> {
        let req = Request::new(Method::GetTransactionCount, Some(vec![address.to_string()]));

        let res: RpcResponse<String> = self.client.post(&self.url).json(&req).send().await?.json().await?;

        let trimmed = res.result.trim_start_matches("0x");

        Ok(u128::from_str_radix(trimmed, 16)?)
    }

    pub async fn get_balances(&self, address: &QkcAddress) -> Result<Balances> {
        let req = Request::new(Method::GetBalances, Some(vec![address.to_string()]));

        let res: RpcResponse<Balances> = self.client.post(&self.url).json(&req).send().await?.json().await?;
        
        Ok(res.result)
    }

    pub async fn get_account_data(&self, address: &QkcAddress) -> Result<AccountData> {
        let req = Request::new(Method::GetAccountData, Some(vec![address.to_string()]));

        let res: RpcResponse<AccountData> = self.client.post(&self.url).json(&req).send().await?.json().await?;
        
        Ok(res.result)
    }

    pub async fn get_blocks_mined_in_recent_256(&self, miner: QkcAddress) -> Result<u64> {
        let mut blocks = vec![self.get_minor_block_by_height(miner.full_shard_key(), Block::Latest).await?];
        let id = u64::from_str_radix(&blocks[0].height[2..], 16)?;

        let block_ids: Vec<String> = (id-255..id).map(|i| format!("0x{}", hex::encode(&i.to_be_bytes()))).collect();

        for id in block_ids {
            blocks.push(self.get_minor_block_by_height(miner.full_shard_key(), Block::Id(id)).await?);
        }

        let mined: u64 = blocks.into_iter().map(|x| x.miner.starts_with(&miner.coinbase()) as u64).sum();

        Ok(mined)
    }

    pub async fn get_root_posw_stake(&self, address: &QkcAddress) -> Result<u128> {
        let abi = include_bytes!("./abi.json").to_vec();
        let contract = ethabi::Contract::load(BufReader::new(abi.as_slice()))?;
        let address_token = ethabi::Token::Address(ethabi::ethereum_types::H160::from_slice(&address.coinbase));
        let data = contract.function("getLockedStakes")?.encode_input(&vec![address_token])?;
        let data = format!("0x{}", hex::encode(data));

        let call = Call {
            from: address.to_string(),
            to: "0x514b43000000000000000000000000000000000100000001".to_string(),
            data,
            value: "0x0".to_string(),
            gas_price: "0x0".to_string(),
            gas: "0xf4240".to_string(),
            gas_token_id: "0x8bb0".to_string(),
            transfer_token_id: "0x8bb0".to_string()
        };

        let req = CallRequest {
            jsonrpc: "2.0".to_string(),
            params: (call, "latest".to_string()),
            method: Method::Call,
            id: 1
        };

        let res: RpcResponse<String> = self.client.post(&self.url).json(&req).send().await?.json().await?;

        if res.result == "0x".to_string() {
            Ok(0u128)
        } else {
            Ok(u128::from_str_radix(&res.result[2..66], 16)?)
        }
    }

    pub async fn get_root_block_by_height(&self, block: Block) -> Result<RootBlockData> {
        let req;
        if let Block::Id(id) = block {
            req = Request::new(Method::GetRootBlockByHeight, Some(vec![id]));
        } else {
            req = Request::new(Method::GetRootBlockByHeight, None);
        }

        let res: RpcResponse<RootBlockData> = self.client.post(&self.url).json(&req).send().await?.json().await?;
        
        Ok(res.result)
    }

    pub async fn get_minor_block_by_height(&self, full_shard_key: String, block: Block) -> Result<MinorBlockData> {
        let req = MinorBlockDataRequest::new(Method::GetMinorBlockByHeight, (full_shard_key, block.get_id(), false));

        let res: RpcResponse<MinorBlockData> = self.client.post(&self.url).json(&req).send().await?.json().await?;

        Ok(res.result)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Call {
    pub from: String,
    pub to: String,
    pub gas_price: String,
    pub gas: String,
    pub data: String,
    pub value: String,
    pub gas_token_id: String,
    pub transfer_token_id: String
}

#[derive(Debug, Serialize)]
struct CallRequest {
    pub jsonrpc: String,
    pub method: Method,
    pub params: (Call, String),
    pub id: usize
}

pub enum Block {
    Latest,
    Id(String)
}

impl Block {
    pub fn get_id(self) -> Option<String> {
        match self {
            Block::Latest => None,
            Block::Id(id) => Some(id)
        }
    }
}



#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcResponse<T> {
    jsonrpc: String,
    pub(crate) result: T,
    id: usize
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct RootBlockData {
    pub id: String,
    pub hash: String,
    pub height: String,
    pub id_prev_block: String,
    pub hash_prev_block: String,
    pub nonce: String,
    pub hash_merkle_root: String,
    pub miner: String,
    pub coinbase: Vec<Balance>,
    pub difficulty: String,
    pub timestamp: String,
    pub size: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct MinorBlockData {
    pub id: String,
    pub height: String,
    pub hash: String,
    pub full_shard_id: String,
    pub chain_id: String,
    pub shard_id: String,
    pub hash_prev_minor_block: String,
    pub id_prev_minor_block: String,
    pub hash_prev_root_block: String,
    pub nonce: String,
    pub hash_merkle_root: String,
    pub hash_evm_state_root: String,
    pub miner: String,
    pub coinbase: Vec<Balance>,
    pub difficulty: String,
    pub extra_data: String,
    pub gas_limit: String,
    pub gas_used: String,
    pub timestamp: String,
    pub size: String,
    pub transactions: Vec<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct NetworkInfo {
    pub network_id: String,
    pub chain_size: String,
    pub shard_sizes: Vec<String>,
    pub syncing: bool,
    pub mining: bool,
    pub shard_server_count: usize
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Balances {
    pub branch: String,
    pub full_shard_id: String,
    pub shard_id: String,
    pub chain_id: String,
    pub balances: Vec<Balance>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Balance {
    pub token_id: String,
    pub token_str: String,
    pub balance: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct AccountData {
    pub primary: AccountShardData,
    pub shards: Option<Vec<AccountShardData>>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct AccountShardData {
    pub full_shard_id: String,
    pub shard_id: String,
    pub chain_id: String,
    pub balances: Vec<Balance>,
    pub transaction_count: String,
    pub is_contract: bool,
}

#[derive(Debug, Serialize)]
struct MinorBlockDataRequest {
    jsonrpc: String,
    method: Method,
    params: (String, Option<String>, bool),
    id: usize
}

impl MinorBlockDataRequest {
    pub(crate) fn new(method: Method, params: (String, Option<String>, bool)) -> Self {

        Self {
            jsonrpc: "2.0".into(),
            method,
            params,
            id: 1usize
        }
    }
}

#[derive(Debug, Serialize)]
struct Request<T: Serialize> {
    jsonrpc: String,
    method: Method,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Vec<T>>,
    id: usize
}

impl<T> Request<T> where T: Serialize {
    pub(crate) fn new(method: Method, params: Option<Vec<T>>) -> Self {

        Self {
            jsonrpc: "2.0".into(),
            method,
            params,
            id: 1usize
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
enum Method {
    NetworkInfo,
    GetTransactionCount,
    GetBalances,
    GetAccountData,
    GetMinorBlockByHeight,
    GetRootBlockByHeight,
    Call
}
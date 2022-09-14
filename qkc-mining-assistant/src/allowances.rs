use anyhow::Result;
use crossbeam_channel::Sender;
use qkc_web3_rs::{
    qkc::{MinorBlockData, RootBlockData},
    types::QkcAddress,
    QkcWeb3,
};
use std::{process::Stdio, sync::Arc};
use tokio::{
    process::{Child, Command},
    task::JoinHandle,
};

use crate::{IniParameters, MinerIni};

static ROOT_ALLOWANCE: u128 = 681_500e18 as u128;

static ALLOWANCES: [u128; 8] = [
    0,
    13_629e18 as u128,
    27_259e18 as u128,
    54_518e18 as u128,
    109_035e18 as u128,
    218_071e18 as u128,
    27_259e18 as u128,
    109_035e18 as u128,
];

#[derive(Debug, Clone)]
pub struct AllowanceThread {
    pub config: Arc<MinerIni>,
    pub sender: Sender<AllowanceInfo>,
    pub balance: u128,
    pub address: Arc<QkcAddress>,
    pub web3: Arc<QkcWeb3>,
    pub config_file: Arc<IniParameters>,
}

#[derive(Debug, Clone)]
pub struct AllowanceInfo {
    pub config: Arc<IniParameters>,
    pub config_ini: Arc<MinerIni>,
    pub address: Arc<QkcAddress>,
    pub difficulty: u128,

    pub used: u32,
    pub allowances: u32,
}

impl AllowanceInfo {
    pub fn ready_to_mine(&self) -> bool {
        if let Some(allowances) = self.config.allowances_to_use {
            self.used <= allowances - self.config.mine_at_free_allowances_from_max
        } else {
            self.used <= self.allowances - self.config.mine_at_free_allowances_from_max
        }
    }

    pub fn continue_mining(&self) -> bool {
        if let Some(allowances) = self.config.allowances_to_use {
            self.used < allowances
        } else {
            self.used < self.allowances
        }
    }

    pub fn difficulty(&self) -> u128 {
        self.difficulty
    }

    pub fn priority(&self) -> u16 {
        self.config.priority
    }

    pub async fn inject_child(&self, child: &mut Option<Child>, miner_exe: &str, miner_dir: &str) -> Result<()> {
        if let Some(child_inner) = child {
            child_inner.kill().await?;
            let child_inner = Command::new(miner_exe)
                .current_dir(miner_dir)
                .args(&self.config.spawn_args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdin(Stdio::piped())
                .spawn()?;

            *child = Some(child_inner);
        } else {
            let child_inner = Command::new(miner_exe)
                .current_dir(miner_dir)
                .args(&self.config.spawn_args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdin(Stdio::piped())
                .spawn()?;

            *child = Some(child_inner);
        }
        Ok(())
    }
}

impl AllowanceThread {
    pub fn new(
        config: Arc<MinerIni>,
        sender: Sender<AllowanceInfo>,
        web3: Arc<QkcWeb3>,
        config_file: Arc<IniParameters>,
    ) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let mut config = AllowanceThread {
                balance: 0,
                address: Arc::new(QkcAddress::new_full(&config.wallet)?),
                web3,
                config,
                sender,
                config_file,
            };

            loop {
                let time = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
                if config.config_file.root_chain {
                    match config.root_allowances_left().await {
                        Ok((used, allowances)) => {
                            info!(
                                "Address {}: {} used / {} allowances (in recent 256 blocks)",
                                config.address.to_string(),
                                used,
                                allowances
                            );

                            config.sender.send(AllowanceInfo {
                                config: config.config_file.clone(),
                                config_ini: config.config.clone(),
                                difficulty: 0,
                                used,
                                allowances,
                                address: config.address.clone(),
                            })?;
                        }
                        Err(e) => {
                            warn!("Error: {e:?}");
                            continue;
                        }
                    };
                } else {
                    match config.allowances_left().await {
                        Ok((used, allowances, difficulty)) => {
                            info!(
                                "Address {}: ({}/{} in recent 256 blocks) difficulty: {:.4}G",
                                config.address.to_string(),
                                used,
                                allowances,
                                difficulty as f64 / 1e9
                            );

                            config.sender.send(AllowanceInfo {
                                config: config.config_file.clone(),
                                config_ini: config.config.clone(),
                                difficulty,
                                used,
                                allowances,
                                address: config.address.clone(),
                            })?;
                        }
                        Err(e) => {
                            warn!("Error: {e:?}");
                            continue;
                        }
                    };
                }
                tokio::time::sleep_until(time).await;
            }
        })
    }

    async fn root_allowances_left(&mut self) -> Result<(u32, u32)> {
        use qkc_web3_rs::qkc::Block;
        if self.balance == 0 {
            self.balance = self.web3.qkc().get_root_posw_stake(&self.address).await?;
        }

        let allowances = self.balance / ROOT_ALLOWANCE;

        let mut blocks = vec![
            self.web3
                .qkc()
                .get_root_block_by_height(Block::Latest)
                .await?,
        ];
        let id = u64::from_str_radix(&blocks[0].height[2..], 16)?;

        let block_ids: Vec<String> = (id - 255..id)
            .map(|i| format!("0x{}", hex::encode(&i.to_be_bytes())))
            .collect();

        let mut handles = Vec::new();

        for chunk in block_ids.chunks(10) {
            let chunk = chunk.to_vec();
            let web3 = self.web3.clone();

            let handle: JoinHandle<Result<Vec<RootBlockData>>> = tokio::spawn(async move {
                let mut blocks = Vec::new();

                for id in chunk {
                    blocks.push(web3.qkc().get_root_block_by_height(Block::Id(id)).await?);
                }

                Ok(blocks)
            });

            handles.push(handle);
        }

        for handle in handles {
            blocks.extend(handle.await??);
        }

        let mined: u32 = blocks
            .into_iter()
            .map(|x| x.miner.starts_with(&self.address.coinbase()) as u32)
            .sum();

        Ok((mined, allowances as u32))
    }

    async fn allowances_left(&mut self) -> Result<(u32, u32, u128)> {
        use qkc_web3_rs::qkc::Block;
        if self.balance == 0 {
            let balance_r = self.web3.qkc().get_account_data(&self.address).await?;
            let balance_r = balance_r
                .primary
                .balances
                .iter()
                .find(|b| b.token_str == "QKC");

            if let Some(b) = balance_r {
                self.balance = u128::from_str_radix(&b.balance[2..], 16)?;
            }
        }

        let allowances = self.balance / ALLOWANCES[self.address.chain_id() as usize];

        let mut blocks = vec![
            self.web3
                .qkc()
                .get_minor_block_by_height(self.address.full_shard_key(), Block::Latest)
                .await?,
        ];
        let difficulty = u128::from_str_radix(&blocks[0].difficulty[2..], 16)?/20;
        let id = u64::from_str_radix(&blocks[0].height[2..], 16)?;

        let block_ids: Vec<String> = (id - 255..id)
            .map(|i| format!("0x{}", hex::encode(&i.to_be_bytes())))
            .collect();

        let mut handles = Vec::new();

        for chunk in block_ids.chunks(10) {
            let chunk = chunk.to_vec();
            let web3 = self.web3.clone();
            let address = self.address.clone();

            let handle: JoinHandle<Result<Vec<MinorBlockData>>> = tokio::spawn(async move {
                let mut blocks = Vec::new();

                for id in chunk {
                    blocks.push(
                        web3.qkc()
                            .get_minor_block_by_height(address.full_shard_key(), Block::Id(id))
                            .await?,
                    );
                }

                Ok(blocks)
            });

            handles.push(handle);
        }

        for handle in handles {
            blocks.extend(handle.await??);
        }

        let mined: u32 = blocks
            .into_iter()
            .map(|x| x.miner.starts_with(&self.address.coinbase()) as u32)
            .sum();

        Ok((mined, allowances as u32, difficulty))
    }
}

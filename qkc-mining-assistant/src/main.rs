use allowances::{AllowanceInfo, AllowanceThread};
use anyhow::Result;
use crossbeam_channel::unbounded;
use qkc_web3_rs::{types::QkcAddress, QkcWeb3};
use serde::Deserialize;
use tokio::{process::Child, task::JoinHandle};
mod allowances;

use std::sync::Arc;

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let configs = Config::load()?;
    let mut handles = Vec::new();

    for config in configs {
        let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            let config_inner = config.clone();
            let web3 = QkcWeb3::new(config.rpc.clone());
            let mut handles = Vec::new();

            let fallback_config = Arc::new(config_inner.fallback_config.clone());
            let fallback_ini = Arc::new(MinerIni::load(&fallback_config.path)?);

            let (sender, receiver) = unbounded();
            let len = config.config_files.len();
            for (index, mut config_file) in config.config_files.into_iter().enumerate() {
                config_file.priority = (len - index) as u16;
                let config_ini = Arc::new(MinerIni::load(&config_file.path)?);
                let web3 = Arc::new(web3.clone());

                handles.push(AllowanceThread::new(
                    config_ini,
                    sender.clone(),
                    web3,
                    Arc::new(config_file),
                ));
            }

            {
                let mut current_info: Option<AllowanceInfo> = None;
                let mut child: Option<Child> = None;
                let mut infos;

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    if !receiver.is_empty() {
                        infos = Vec::new();
                        while !receiver.is_empty() {
                            infos.push(receiver.recv()?);
                        }

                        let available_infos: Vec<&AllowanceInfo> =
                            infos.iter().filter(|i| i.ready_to_mine()).collect();

                        if let Some(mut current_info_ref) = current_info.clone() {
                            let current_info_update = infos
                                .iter()
                                .find(|i| i.address.to_string() == current_info_ref.address.to_string());
                            let infos: Vec<&AllowanceInfo> = available_infos;

                            if let Some(update) = current_info_update {
                                if !update.continue_mining() {
                                    info!("Stopping current miner for {}: {} used / {} allowances (in recent 256 blocks)", update.address.to_string(), update.used, update.allowances);
                                    if let Some(child_mut) = child.as_mut() {
                                        child_mut.kill().await?;
                                        child = None;
                                        current_info = None;
                                    }
                                } else {
                                    current_info = Some(update.clone());
                                    current_info_ref = update.clone();
                                }
                            }

                            let mut replacement = infos.into_iter().filter(|i| i.difficulty() < current_info_ref.difficulty()).collect::<Vec<&AllowanceInfo>>(); //.min_by_key(|i| i.difficulty());
                            replacement.sort_by_key(|k| k.difficulty());
                            let replacement_info;
                            if let Some(first) = replacement.first() {
                                replacement_info = replacement.iter().filter(|i| i.difficulty() == first.difficulty()).max_by_key(|i| i.priority());
                            } else {
                                replacement_info = None;
                            } 

                            if let Some(replacement) = replacement_info {

                                if current_info_ref.difficulty() > replacement.difficulty() || (current_info_ref.difficulty() == replacement.difficulty() && current_info_ref.priority() < replacement.priority()) {
                                    info!("Replacing current miner for {} ({}/{}) difficulty {:.4}G, with {} ({}/{}) difficulty {:.4}G", current_info_ref.address.to_string(), current_info_ref.used, current_info_ref.allowances, current_info_ref.difficulty() as f64 / 1e9 as f64, replacement.address.to_string(), replacement.used, replacement.allowances, replacement.difficulty() as f64 / 1e9 as f64);
                                    replacement
                                        .inject_child(&mut child, &config_inner.miner_exe, &config_inner.miner_dir)
                                        .await?;
                                    current_info = Some(replacement.clone().clone());
                                } else if current_info.is_none() {
                                    info!(
                                        "Initializing miner for {} ({}/{}) difficulty {:.4}G",
                                        replacement.address.to_string(),
                                        replacement.used,
                                        replacement.allowances,
                                        replacement.difficulty() as f64 / 1e9 as f64
                                    );
                                    replacement
                                        .inject_child(&mut child, &config_inner.miner_exe, &config_inner.miner_dir)
                                        .await?;
                                    current_info = Some(replacement.clone().clone());
                                }
                            } else if current_info.is_none() {
                                info!("Initializing miner for fallback");

                                let info = AllowanceInfo {
                                    config: fallback_config.clone(),
                                    config_ini: fallback_ini.clone(),
                                    address: Arc::new(QkcAddress::new_full(&fallback_ini.wallet)?),
                                    difficulty: 0,
                                    used: 0,
                                    allowances: 0,
                                };
                                info.inject_child(&mut child, &config_inner.miner_exe, &config_inner.miner_dir)
                                    .await?;
                                current_info = Some(info);
                            }
                        } else if available_infos.len() == 0 && current_info.is_none() {
                            info!("Initializing miner for fallback");

                            let info = AllowanceInfo {
                                config: fallback_config.clone(),
                                config_ini: fallback_ini.clone(),
                                address: Arc::new(QkcAddress::new_full(&fallback_ini.wallet)?),
                                difficulty: 0,
                                used: 0,
                                allowances: 0,
                            };
                            info.inject_child(&mut child, &config_inner.miner_exe, &config_inner.miner_dir)
                                .await?;
                            current_info = Some(info);
                        } else if current_info.is_none() {
                            let mut replacement = infos;
                            replacement.sort_by_key(|k| k.difficulty());
                            let replacement_info;
                            if let Some(first) = replacement.first() {
                                replacement_info = replacement.iter().filter(|i| i.difficulty() == first.difficulty()).max_by_key(|i| i.priority());
                            } else {
                                replacement_info = None;
                            }

                            if let Some(replacement) = replacement_info {
                                info!(
                                    "Initializing miner for {} ({}/{}) difficulty {:.4}G",
                                    replacement.address.to_string(),
                                    replacement.used,
                                    replacement.allowances,
                                    replacement.difficulty() as f64/1e9 as f64
                                );
                                replacement
                                    .inject_child(&mut child, &config_inner.miner_exe, &config_inner.miner_dir)
                                    .await?;
                                current_info = Some(replacement.clone().clone())
                            }
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IniFile {
    ethash: MinerIni,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct MinerIni {
    pub wallet: String,
}

impl MinerIni {
    pub fn load(config_file: &String) -> Result<Self> {
        let file = std::fs::read_to_string(config_file)?;

        Ok(serde_ini::from_str::<IniFile>(&file)?.ethash)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    pub rpc: String,
    pub miner_dir: String,
    pub miner_exe: String,
    pub fallback_config: IniParameters,
    pub config_files: Vec<IniParameters>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IniParameters {
    pub spawn_args: Vec<String>,
    pub path: String,

    #[serde(default)]
    pub root_chain: bool,
    pub allowances_to_use: Option<u32>,
    pub mine_at_free_allowances_from_max: u32,

    #[serde(skip_deserializing)]
    pub priority: u16
}

impl Config {
    pub fn load() -> Result<Vec<Config>> {
        let file = std::fs::read_to_string("config.json")?;

        Ok(serde_json::from_str(&file)?)
    }
}

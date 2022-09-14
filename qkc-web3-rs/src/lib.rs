pub use ethabi;
pub mod types;
pub mod qkc;

use qkc::Qkc;

#[derive(Debug, Clone)]
pub struct QkcWeb3 {
    qkc: Qkc
}

impl QkcWeb3 {
    pub fn new(url: String) -> Self {
        Self {
            qkc: Qkc {
                client: reqwest::Client::new(),
                url
            }
        }
    }

    pub fn qkc(&self) -> &Qkc {
        &self.qkc
    }
}

#[cfg(test)]
mod test {
    use tokio::task::JoinHandle;

    use crate::{QkcWeb3, types::QkcAddress};

    #[test]
    fn address_test() {
        let address_with_chain = QkcAddress::new("0xF0c9A075c4386ab8F08CF4529FDF77F6D2748d02", 7, 0).unwrap();

        println!("{}", address_with_chain.to_string());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 16)]
    async fn request_test() {
        use crate::qkc::Block;
        let address = QkcAddress::new("0x13d041434910aD2C1893c6A77537B16Cb7b8Ef5b", 0, 0).unwrap();
        let web3 = QkcWeb3::new("http://jrpc.mainnet.quarkchain.io:38391".into());
        let res = web3.qkc().network_info().await.unwrap();

        println!("{res:?}");

        let res = web3.qkc().get_transaction_count(&address).await.unwrap();

        println!("{res:?}");

        let res = web3.qkc().get_balances(&address).await.unwrap();

        println!("{res:?}");

        let res = web3.qkc().get_account_data(&address).await.unwrap();

        println!("{res:?}");

        let res = web3.qkc().get_root_block_by_height(Block::Latest).await.unwrap();

        println!("{res:?}");

        let res = web3.qkc().get_root_posw_stake(&address).await.unwrap();

        println!("{res}");
    }

    #[tokio::test]
    async fn request_test_multi_thread() {
        let handles = (0..10000).map(|_| {
            //let address = QkcAddress::new("0x13d041434910aD2C1893c6A77537B16Cb7b8Ef5b", 3, 0).unwrap();;
            let web3 = QkcWeb3::new("http://jrpc.mainnet.quarkchain.io:38391".into());
            tokio::spawn(async move {

                loop {
                    let res = web3.qkc().network_info().await.unwrap();
                    println!("{:?}", res);
                }
            })
        }).collect::<Vec<JoinHandle<()>>>();

        for handle in handles {
            handle.await.unwrap()
        }
    }
}
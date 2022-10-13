use std::{env, sync::Arc};

mod protocol;
mod error;
mod broker;

use broker::Broker;
use futures::StreamExt;
use smol::{net::TcpListener, lock::Mutex};

use crate::error::Result;


const ADDR: &'static str = "127.0.0.1:8080";


fn main() -> Result<()> {
    // let test = ProtocolPackage::ChannelConnectionRequest { channel: "channel".to_string() };
    // let encoded: Vec<u8> = bincode::serialize(&test)?;
    // let decoded: ProtocolPackage = bincode::deserialize(&encoded[..])?;
    let args: Vec<String> = env::args().collect();
    if args[1] == "listen" {
        println!("Listening on {}", ADDR);
        // let mut server = ChatServer::new(ADDR.to_string())?;
        // server.listen()?;
    } else {
        println!("Streaming to {}", ADDR);
        // start_client(ADDR.to_string())?;
        todo!();
    }

    Ok(())
}


struct ChatServer {
    socket: TcpListener,
    broker: Arc<Mutex<Broker>>,
}

impl ChatServer {
    async fn new(addr: String) -> Result<Self> {
        Ok(ChatServer {
            socket: TcpListener::bind(addr).await?,
            broker: Arc::new(Mutex::new(Broker::new())),
        })
    }

    async fn listen(&mut self) -> Result<()> {
        self.socket
            .incoming()
            .for_each_concurrent(None, |stream| async {
                let guard = self.broker.lock_arc().await;               
            })
        .await;
        // for stream in self.socket.incoming() {
        //     let mut buffer = Vec::new();
        //     stream?.read_to_end(&mut buffer)?;
        //     println!("{:?}", buffer);
        //     todo!();
        // }

        Ok(())
    }
}

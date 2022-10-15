use std::{env, sync::Arc};

mod protocol;
mod error;
mod broker;

use broker::Broker;
use error::GResult;
use futures::TryStreamExt;
use protocol::{HasServerConnection, server::ServerSideConnectionFMS};
use smol::{net::TcpListener, lock::Mutex};
use std::io;
use std::error::Error;

use crate::error::Result;


const ADDR: &'static str = "127.0.0.1:8080";


fn main() -> std::result::Result<(), Box<dyn Error>> {
    smol::block_on(async {
        let args: Vec<String> = env::args().collect();
        if args[1] == "listen" {
            println!("Listening on {}", ADDR);
            // let mut chat_server = ChatServer::new(ADDR.to_string()).await.unwrap();
            // chat_server.listen().await.unwrap();
            // let mut server = ChatServer::new(ADDR.to_string())?;
            // server.listen()?;
        } else {
            println!("Streaming to {}", ADDR);
            // start_client(ADDR.to_string())?;
            todo!();
        }

    });
    
    Ok(())
}


struct ChatServer {
    socket: TcpListener,
    broker: Arc<Mutex<Broker>>,
}

impl ChatServer {
    async fn new(addr: String) -> GResult<Self> {
        Ok(ChatServer {
            socket: TcpListener::bind(addr).await?,
            broker: Arc::new(Mutex::new(Broker::new())),
        })
    }

    async fn listen(&mut self) -> GResult<()> {
        let broker = &self.broker;
        self.socket
            .incoming()
            .try_for_each_concurrent(None, |stream| async move {
                let server_side_fsm = ServerSideConnectionFMS::new(broker, stream);
                let server_side_fsm = server_side_fsm.authenticate().await;
                
                Ok(())
                // match package {
                //     ServerConnectionRequest { username } => {
                //         {
                //             let guard = broker.lock_arc().await;               
                //             if guard.has_username(&username) {
                //                 return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Username already exists"));
                //             }
                //         }
                //         let reply = ServerConnectionAccept;
                //         let reply = stream.send_package_and_receive(reply).await.unwrap(); // TODO no unwrap
                //         // TODO this block should be handled by the code in the next TODO comment
                //         match reply {
                //             ChannelConnectionRequest { channel } => {
                //                 let mut guard = broker.lock_arc().await;               
                //                 guard.subscribe(channel, stream.clone());
                //             }
                //             _ => {
                //                 return Err(io::Error::new(io::ErrorKind::Other, "Malformed message"))
                //             }
                //         }
                //         
                //         // TODO read incoming messages from stream and handle accordingly 
                //         // this can be done, I think, by implementing the Stream trait from the futures crate for TcpConnection and then using 
                //         // try_for_each_concurrent from the StreamExt trait
                //
                //
                //     }
                //     _ => {
                //         return Err(io::Error::new(io::ErrorKind::Other, "Malformed message"))
                //     }
                // }
                // Ok(())
            })
        .await?;

        Ok(())
    }
}

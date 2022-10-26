use std::{env, sync::Arc};
use std::error::Error;

mod protocol;
mod broker;

use broker::Broker;
use futures::{StreamExt, TryStreamExt};
use smol::{net::TcpListener, lock::Mutex};

use crate::protocol::{HasServerConnection, ProtocolPackage, server::ClientConnection};
use crate::protocol::server::{ServerSideConnectionSM, SharedContext};
use rust_state_machine::{StateMachineAsync, StatefulAsyncStateMachine};


const ADDR: &str = "127.0.0.1:8080";


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
    async fn new(addr: String) -> std::io::Result<Self> {
        Ok(ChatServer {
            socket: TcpListener::bind(addr).await?,
            broker: Arc::new(Mutex::new(Broker::new())),
        })
    }

    async fn listen<'a>(&mut self) {
        let broker = &self.broker;
        self.socket
            .incoming()
            .for_each_concurrent(None, |stream| async move {
                // let server_side_fsm = ServerSideConnectionFMS::new(broker, stream);
                let mut stream = stream.unwrap();
                let shared = SharedContext { broker: broker.clone() };
                let start_state = ClientConnection{ socket: stream.clone() };
                let mut server_side_sm: StateMachineAsync<ServerSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
                while let Ok(message) = stream.receive_package().await {
                    let res = server_side_sm.transition(message).await;
                    println!("{:?}", res);
                }

                // let temp = server_side_fsm.listen().await;
                // match temp {
                //     Ok(val) => println!("succeeded: {:?}", val),
                //     Err(err) => {
                //         // do this is on generic FSM trait object
                //         match err {
                //             ServerFailEdges::Rejected(mut fsm, error) => {
                //                 let temp: ServerResult<'a, (), ClientConnection> = fsm.send_package(ProtocolPackage::Rejection { reason: error }).await;
                //                 temp?;
                //             },
                //             ServerFailEdges::MalformedPackage(mut fsm) => {
                //                 let temp: ServerResult<'a, (), ClientConnection> = fsm.send_package(ProtocolPackage::Rejection { reason: ErrorType::MalformedPackage }).await;
                //                 temp?;
                //             }
                //             ServerFailEdges::Disconnected => {},
                //             ServerFailEdges::IoError(err) => {/* TODO should close connection */ }
                //             ServerFailEdges::DeserializeError(err) => {/* TODO should close connection */ }
                //         }
                //     } 
                // }

                // println!("something happened: {:?}", temp);
            })
        .await;
    }
}

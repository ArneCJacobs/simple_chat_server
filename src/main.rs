use std::time::Duration;
use std::{env, sync::Arc};
use std::error::Error;

mod protocol;
mod broker;

use broker::Broker;
use futures::{StreamExt, TryStreamExt};
use smol::Timer;
use smol::{net::TcpListener, lock::Mutex};

use crate::protocol::client::{ClientSideConnectionSM, Input, NotConnected, Shared};
use crate::protocol::{HasServerConnection, ProtocolPackage, server::ClientConnection};
use crate::protocol::server::{ServerSideConnectionSM, SharedContext};
use rust_state_machine::{StateMachineAsync, StatefulAsyncStateMachine};


const ADDR: &str = "127.0.0.1:8080";


fn main() -> std::result::Result<(), Box<dyn Error>> {
    smol::block_on(async {
        let args: Vec<String> = env::args().collect();
        if args[1] == "listen" {
            println!("Listening on {}", ADDR);
            let mut server = ChatServer::new(ADDR.to_string()).await.unwrap();
            server.listen().await;
        } else {
            Timer::after(Duration::from_secs(1)).await;
            println!("Streaming to {}", ADDR);
            let shared = Shared;
            let start_state = NotConnected;
            let mut client: StateMachineAsync<ClientSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
            let commands = vec![
                Input::ConnectServer(ADDR.to_string()),
                Input::Authenticate("Steam".to_string()),
                Input::GetChannelsList,
                Input::Disconnect,
            ];

            for command in commands {
                println!("SENDING COMMAND: {:?}" ,command);
                let output = client.transition(command.clone()).await;
                println!("COMMAND: {:?}, RESPONSE: {:?}", command, output);
            }
            // start_client(ADDR.to_string())?;
            println!("DONE");
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

    async fn listen(&mut self) {
        let broker = &self.broker;
        self.socket
            .incoming()
            .for_each_concurrent(None, |stream| async move {
                let mut stream = stream.unwrap();
                println!("NEW CONNECTION: {:?}", stream.peer_addr());
                let shared = SharedContext { broker: broker.clone() };
                let start_state = ClientConnection{ socket: stream.clone() };
                let mut server_side_sm: StateMachineAsync<ServerSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
                while let Ok(message) = stream.receive_package().await {
                    println!("RECEIVED MESSAGE: {:?}", message);
                    let res = server_side_sm.transition(message).await;
                    println!("RESPONSE: {:?}", res);
                }
            })
        .await;
    }
}

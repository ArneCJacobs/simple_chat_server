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
use crate::protocol::server::{ClientChannelConnection, Reaction, ServerSideConnectionSM, ServerSideConnectionSMStates, SharedContext};
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
            Timer::after(Duration::from_secs_f64(0.1)).await;
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
                let peer_addr = stream.peer_addr();
                println!("NEW CONNECTION: {:?}", peer_addr);
                let shared = SharedContext { broker: broker.clone() };
                let start_state = ClientConnection{ socket: stream.clone() };
                let mut server_side_sm: StateMachineAsync<ServerSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
                let mut res = None;
                // TODO: move all this to internal logic, perhaps provide a consume function which
                // is called in a loop
                // TODO: implement a post process function for SM where certain states before 
                while let Ok(message) = stream.receive_package().await {
                    println!("RECEIVED MESSAGE: {:?}", message);
                    res = server_side_sm.transition(message).await;
                    println!("RESPONSE: {:?}", res);
                    if let Some(Reaction::LostConnecion) = res {
                        // TODO: this will never happen, the only state that is ever given with
                        // Reaction::LostConnecion is Disconnected, maybe add some state to
                        // disconnected? 
                        if let Some(ServerSideConnectionSMStates::ClientChannelConnection(state)) = server_side_sm.get_current_state() {
                            let ClientChannelConnection { channel, broker, username, .. } = state;
                            let message_text = format!("{} LOST CONNECTION", username);
                            let message = ProtocolPackage::ChatMessageSend{ message: message_text.to_string() };
                            let mut guard = broker.lock_arc().await;
                            guard.notify(channel.to_owned(), message).await;
                        }
                    }
                    if res.is_none() {
                        break;
                    }
                }

                if let Some(Reaction::LostConnecion) = res {
                    println!("CONNECTION LOST: {:?}", peer_addr);
                    // TODO: send message to channel if it was connected to one
                } else {
                    println!("CONNECTION ENDED: {:?}", peer_addr);
                }
            })
        .await;
    }
}

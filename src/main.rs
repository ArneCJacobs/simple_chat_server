use std::time::Duration;
use std::{env, sync::Arc};
use std::error::Error;

mod protocol;
mod broker;

use broker::Broker;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::Level;

use crate::protocol::client::{ClientSideConnectionSM, Input, NotConnected, Shared};
use crate::protocol::server::{ClientConnection, Reaction, ServerSideConnectionSM, SharedContext};
use rust_state_machine::{StateMachineAsync, StatefulAsyncStateMachine};

const ADDR: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .without_time()
        .with_target(false)
        .finish();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;

    let args: Vec<String> = env::args().collect();
    // TODO: separate bins
    if args[1] == "listen" {
        tracing::info!("Listening on {}", ADDR);
        let mut server = ChatServer::new(ADDR.to_string()).await.unwrap();
        server.listen().await;
    } else {
        tokio::time::sleep(Duration::from_secs_f64(0.1)).await;
        tracing::info!("Streaming to {}", ADDR);
        let shared = Shared;
        let start_state = NotConnected;
        let mut client: StateMachineAsync<ClientSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
        let commands = vec![
            Input::ConnectServer(ADDR.to_string()),
            Input::Authenticate("Steam".to_string()),
            Input::GetChannelsList,
            Input::ConnectChannel("Welcome".to_string()),
            // Input::SendMessage("Hello Chat".to_string()),
            Input::Disconnect,
        ];

        for command in commands {
            tracing::info!("SENDING COMMAND: {:?}" ,command);
            let output = client.transition(command.clone()).await;
            tracing::info!("RESPONSE: {:?}", output);
        }
        // start_client(ADDR.to_string())?;
        tracing::debug!("DONE");
    }
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
        loop {
            let (socket, _) = self.socket.accept().await.unwrap();
            let broker = self.broker.clone();
            tokio::spawn(async move {
                ChatServer::spawn_handler(broker, socket).await;
            });
        }
    }

    async fn spawn_handler(broker: Arc<Mutex<Broker>>, stream: TcpStream) {
        let peer_addr = stream.peer_addr();
        tracing::info!("NEW CONNECTION: {:?}", peer_addr);
        let shared = SharedContext { broker: broker.clone() };
        let stream = Arc::new(Mutex::new(stream));
        let start_state = ClientConnection{ socket: stream };
        let mut server_side_sm: StateMachineAsync<ServerSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
        let mut result = None;
        loop {
            let res = server_side_sm.transition(()).await;
            tracing::debug!("RESPONSE: {:?}", res);
            if res.is_none() {
                break;
            } else {
                result = res;
            }
        } 
        if let Some(Reaction::LostConnecion) = result {
            tracing::info!("CONNECTION LOST: {:?}", peer_addr);
        } else if let Some(Reaction::Disconnected) = result {
            tracing::info!("CONNECTION ENDED: {:?}", peer_addr);
        }
    }
}

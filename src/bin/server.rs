use rust_state_machine::{StateMachineAsync, StatefulAsyncStateMachine};
use simple_chat_protocol::broker::Broker;
use simple_chat_protocol::protocol::server::{ClientConnection, SharedContext, ServerSideConnectionSM, Reaction};
use simple_chat_protocol::protocol::util::split_stream::SplitStream;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::Level;
use std::error::Error;
use std::sync::Arc;

// use simple_chat_protocol::broker::Broker;

// use crate::protocol::server::{ClientConnection, Reaction, ServerSideConnectionSM, SharedContext};
//
//
const ADDR: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .without_time()
        .with_target(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    tracing::info!("Listening on {}", ADDR);
    let mut server = ChatServer::new(ADDR.to_string()).await.unwrap();
    server.listen().await;

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
        let stream = SplitStream::new(stream).await;
        let start_state = ClientConnection{ socket: stream };
        let mut server_side_sm: StateMachineAsync<ServerSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
        let mut result = None;
        loop {
            let res = server_side_sm.transition(()).await;
            tracing::info!("RESPONSE: {:?}", res);
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

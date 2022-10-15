use std::{env, sync::Arc};

mod protocol;
mod error;
mod broker;

use broker::Broker;
use error::GResult;
use futures::TryStreamExt;
use protocol::server::ServerSideConnectionFMS;
use smol::{net::TcpListener, lock::Mutex};
use std::error::Error;


const ADDR: &'static str = "127.0.0.1:8080";


fn main() -> std::result::Result<(), Box<dyn Error>> {
    smol::block_on(async {
        let args: Vec<String> = env::args().collect();
        if args[1] == "listen" {
            println!("Listening on {}", ADDR);
            let mut chat_server = ChatServer::new(ADDR.to_string()).await.unwrap();
            chat_server.listen().await.unwrap();
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
                let temp = server_side_fsm.listen().await?;
                println!("something happened: {:?}", temp);
                Ok(())
            })
        .await?;

        Ok(())
    }
}

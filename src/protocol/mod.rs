use serde::{Serialize, Deserialize};
use smol::{net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};
use async_trait::async_trait;

use crate::error::Result;

pub mod client;
pub mod server;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum ProtocolPackage {
    ServerConnectionRequest{ username: String },
    ServerConnectionAccept,

    InfoListChannelsRequest,
    InfoListChannelsReply { channels: Vec<String> },

    ChannelConnectionRequest{ channel: String },
    ChannelConnectionRequestAccept,
    ChannelConnectionRequestDeny,

    ChatMessageSend { message: String },
    ChatMessageSendAccept,
    ChatMessageSendDeny { error: String },
    ChatMessageReceive { username: String, message: String},

    Disconnect,
}

#[async_trait]
pub trait HasServerConnection<'a, T> {
    fn get_server_socket(&mut self) -> &mut TcpStream;

    async fn send_package_and_receive(&mut self, message: ProtocolPackage) -> Result<'a, T> {
        self.send_package(message).await?;
        self.receive_package().await
    }

    async fn receive_package(&mut self) -> Result<'a, T> {
        let socket = self.get_server_socket();
        let mut buffer = Vec::new();
        socket.read_to_end(&mut buffer).await?;
        let received_message: ProtocolPackage = bincode::deserialize(&buffer[..])?;
        Ok(received_message)
    }

    async fn send_package(&mut self, message: ProtocolPackage) -> Result<'a, > {
        let socket = self.get_server_socket();
        let serialized = bincode::serialize(&message)?;
        socket.write_all(&serialized).await?;
        Ok(())
    }
}

impl HasServerConnection for TcpStream {
    fn get_server_socket(&mut self) -> &mut TcpStream {
        self
    }
}

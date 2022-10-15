use std::fmt::Debug;

use serde::{Serialize, Deserialize};
use smol::{net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};
use async_trait::async_trait;
use std::result::Result as StdResult;

use crate::error::ServerResult;

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

    DisconnectNotification,
}

#[async_trait]
pub trait HasServerConnection<E>
    where E: From<Box<bincode::ErrorKind>> + From<std::io::Error> {
    fn get_server_socket(&mut self) -> &mut TcpStream;

    async fn send_package_and_receive(&mut self, message: ProtocolPackage) -> StdResult<ProtocolPackage, E> {
        self.send_package(message).await?;
        self.receive_package().await
    }

    async fn receive_package(&mut self) -> StdResult<ProtocolPackage, E> {
        let socket = self.get_server_socket();
        let mut buffer = Vec::new();
        socket.read_to_end(&mut buffer).await?;
        let received_message: ProtocolPackage = bincode::deserialize(&buffer[..])?;
        Ok(received_message)
    }

    async fn send_package(&mut self, message: ProtocolPackage) -> StdResult<(), E> {
        let socket = self.get_server_socket();
        let serialized = bincode::serialize(&message)?;
        socket.write_all(&serialized).await?;
        Ok(())
    }
}

impl<E> HasServerConnection<E> for TcpStream
    where E: From<Box<bincode::ErrorKind>> + From<std::io::Error> {
    fn get_server_socket(&mut self) -> &mut TcpStream {
        self
    }
}

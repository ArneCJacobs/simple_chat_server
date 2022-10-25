use std::fmt::Debug;
use std::result::Result as StdResult;

use serde::{Serialize, Deserialize};
use smol::{net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};
use async_trait::async_trait;

pub mod client;
pub mod server;

#[derive(Serialize, Deserialize, Debug)]
pub enum ProtocolPackage {
    ServerAuthenticationRequest{ username: String },

    InfoListChannelsRequest,
    InfoListChannelsReply { channels: Vec<String> },

    ChannelConnectionRequest{ channel: String },

    ChatMessageSend { message: String },
    ChatMessageSendAccept,

    ChatMessageReceive { username: String, message: String},

    DisconnectNotification,

    Accept,
    Deny { message: String },
}

pub enum SendReceiveError {
    BinError(Box<bincode::ErrorKind>),
    IoError(std::io::Error),
}

impl From<Box<bincode::ErrorKind>> for SendReceiveError {
    fn from(error: Box<bincode::ErrorKind>) -> Self {
        SendReceiveError::BinError(error) 
    }
}

impl From<std::io::Error> for SendReceiveError {
    fn from(error: std::io::Error) -> Self {
        SendReceiveError::IoError(error)
    }
}

#[async_trait]
pub trait HasServerConnection
{
    fn get_server_socket(&mut self) -> &mut TcpStream;

    async fn send_package_and_receive(&mut self, message: ProtocolPackage) -> StdResult<ProtocolPackage, SendReceiveError> {
        self.send_package(message).await?;
        self.receive_package().await
    }

    async fn receive_package(&mut self) -> StdResult<ProtocolPackage, SendReceiveError> {
        let socket = self.get_server_socket();
        let mut buffer = Vec::new();
        socket.read_to_end(&mut buffer).await?;
        let received_message: ProtocolPackage = bincode::deserialize(&buffer[..])?;
        Ok(received_message)
    }

    async fn send_package(&mut self, message: ProtocolPackage) -> StdResult<(), SendReceiveError> {
        let socket = self.get_server_socket();
        let serialized = bincode::serialize(&message)?;
        socket.write_all(&serialized).await?;
        Ok(())
    }
}

impl HasServerConnection for TcpStream
{
    fn get_server_socket(&mut self) -> &mut TcpStream {
        self
    }
}

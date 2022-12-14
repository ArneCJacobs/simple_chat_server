use tokio::{io::{AsyncWriteExt, AsyncReadExt}, sync::{Mutex, mpsc::Receiver}, net::tcp::{OwnedReadHalf, OwnedWriteHalf}, task::{JoinHandle, JoinError}};
use tokio::net::TcpStream;
use std::{fmt::Debug, result::Result as StdResult, sync::Arc};

use serde::{Serialize, Deserialize};
use async_trait::async_trait;

use crate::broker::BrokerError;

pub mod client;
pub mod server;
pub mod util; 

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolPackage {
    ServerAuthenticationRequest{ username: String },

    InfoListChannelsRequest,
    InfoListChannelsReply { channels: Vec<String> },

    ChannelConnectionRequest{ channel: String },
    ChannelDisconnectNotification,

    ChatMessageSend { message: String }, // From client to server

    ChatMessageReceive { username: String, message: String}, // from server to client

    DisconnectNotification,
    MalformedPackage,

    Accept,
    Deny { error: BrokerError },
}

#[derive(Debug)]
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
pub trait ProtocolPackageSender {
    async fn send_package(&mut self, message: ProtocolPackage) -> StdResult<(), SendReceiveError>;
}

#[async_trait]
pub trait ProtocolPackageReader {
    async fn receive_package(&mut self) -> StdResult<ProtocolPackage, SendReceiveError>;
}

#[async_trait]
pub trait HasServerConnection: ProtocolPackageReader + ProtocolPackageSender {
    async fn send_package_and_receive(&mut self, message: ProtocolPackage) -> StdResult<ProtocolPackage, SendReceiveError> {
        self.send_package(message).await?;
        self.receive_package().await
    }
}

macro_rules! impl_protocol_package_sender {
    ( $($t:ty),* ) => {
    $(
        #[async_trait]
        impl ProtocolPackageSender for $t {
            async fn send_package(&mut self, message: ProtocolPackage) -> StdResult<(), SendReceiveError> {
                tracing::debug!("SENDING PACKAGE {:?}", message);
                let serialized = bincode::serialize(&message)?;
                let len: u64 = serialized.len() as u64;
                self.write_all(&len.to_le_bytes()).await?;
                self.write_all(&serialized).await?;
                Ok(())
            }
        }
    )*
    }
}

impl_protocol_package_sender!(TcpStream, OwnedWriteHalf);

macro_rules! impl_protocol_package_reader {
    ( $($t:ty),* ) => {
    $(
        #[async_trait]
        impl ProtocolPackageReader for $t {
            async fn receive_package(&mut self) -> StdResult<ProtocolPackage, SendReceiveError> {
                tracing::debug!("== RECEIVING PACKAGE == ");
                let len: u64 = self.read_u64_le().await?;
                tracing::debug!("DATA HAS LENGTH: {:?}", len);
                if len > 1000 {
                panic!("The received message has supposed length {}, which is larger then the maximum allowed length", len);
            }
                let mut buffer: Vec<u8> = vec![0; len.try_into().unwrap()];
                self.read_exact(&mut buffer).await?;
                let received_message: ProtocolPackage = bincode::deserialize(&buffer[..])?;
                tracing::debug!("== RECEIVED PACKAGE {:?} ==", received_message);
                Ok(received_message)
            }
        }
    )*
    }
}

impl_protocol_package_reader!(TcpStream, OwnedReadHalf);

impl HasServerConnection for TcpStream {}

// TODO: make a handler for a TCP stream see: https://tokio.rs/tokio/tutorial/shared-state#spawn-a-task-to-manage-the-state-and-use-message-passing-to-operate-on-it
// TODO: replace usages of Connection with split_stream;
pub type Connection = Arc<Mutex<TcpStream>>;

#[async_trait]
impl HasServerConnection for Connection {
    async fn send_package_and_receive(&mut self, message: ProtocolPackage) -> StdResult<ProtocolPackage, SendReceiveError>
    {
        let mut socket = self.lock().await;
        socket.send_package_and_receive(message).await
    }
}

#[async_trait]
impl ProtocolPackageReader for Connection {
    async fn receive_package(&mut self) -> StdResult<ProtocolPackage, SendReceiveError> {
        let mut socket = self.lock().await;
        socket.receive_package().await
    }
}

#[async_trait]
impl ProtocolPackageSender for Connection {
    async fn send_package(&mut self, message: ProtocolPackage) -> StdResult<(), SendReceiveError> {
        let mut socket = self.lock().await;
        socket.send_package(message).await
    }
}

// TODO: move this and trait impls into different file
pub struct FilteredTcpStream {
    socket: OwnedWriteHalf,
    receiver: Receiver<Result<ProtocolPackage, SendReceiveError>>
}
impl HasServerConnection for FilteredTcpStream {}

#[async_trait]
impl ProtocolPackageReader for FilteredTcpStream {
    async fn receive_package(&mut self) -> StdResult<ProtocolPackage, SendReceiveError> {
        use std::io;
        match self.receiver.recv().await {
            Some(result) => result,
            None => Err(SendReceiveError::IoError(io::Error::new(io::ErrorKind::NotConnected, "Receiver channel disconnected")))
        }
    }
}

#[async_trait]
impl ProtocolPackageSender for FilteredTcpStream {
    async fn send_package(&mut self, message: ProtocolPackage) -> StdResult<(), SendReceiveError> {
        self.socket.send_package(message).await
    }
}

impl FilteredTcpStream {
    pub fn get_unfiltered(self) -> OwnedWriteHalf {
        std::mem::drop(self.receiver);
        self.socket
    }

    pub async fn get_tcp_connection(self, handle: JoinHandle<OwnedReadHalf>) -> Result<TcpStream, JoinError> {
        let writer_half = self.get_unfiltered();
        let reader_hald = handle.await?; 
        Ok(writer_half.reunite(reader_hald).unwrap())
    }
}

// TODO: move into different file
#[macro_export]
macro_rules! impl_send_receive {
    ( $reaction:ty, $not_connected:ident, $($x:ty),* ) => {
        $(
            impl<Edges> ::rust_state_machine::ToStatesAndOutput<$x, Edges, $reaction> for SendReceiveError 
            where Edges: From<$not_connected> + From<$x>
            {
                fn context(self, state: $x) -> (Edges, $reaction) {
                    match self {
                        SendReceiveError::IoError(_) => ($not_connected.into(), self.into()),
                        SendReceiveError::BinError(_) => (state.into(), self.into()),
                    }

                }
            }

            impl<Edges> ::rust_state_machine::ToStatesAndOutput<$x, Edges, $reaction> for std::io::Error 
            where Edges: From<$not_connected> + From<$x>
            {
                fn context(self, state: $x) -> (Edges, $reaction) {
                    ToStatesAndOutput::context(SendReceiveError::IoError(self), state)
                }
            }
        )*
    };
}

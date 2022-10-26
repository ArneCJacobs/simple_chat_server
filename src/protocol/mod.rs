use std::fmt::Debug;
use std::result::Result as StdResult;

use rust_state_machine::ToStatesAndOutput;
use serde::{Serialize, Deserialize};
use smol::{net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};
use async_trait::async_trait;

use crate::broker::ErrorType;

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
    Deny { error: ErrorType },
}

pub enum SendReceiveError {
    BinError(Box<bincode::ErrorKind>),
    IoError(std::io::Error),
}

pub struct Wrapper<E: Into<SendReceiveError>>(E);

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
        // println!("RECEIVING PACKAGE");
        let socket = self.get_server_socket();
        let mut length_buffer = [0; 8];
        socket.read_exact(&mut length_buffer).await?;
        let len: u64 = u64::from_le_bytes(length_buffer);
        // println!("DATA HAS LENGTH: {:?}", len);
        let mut buffer: Vec<u8> = vec![0; len.try_into().unwrap()];
        socket.read_exact(&mut buffer).await?;
        let received_message: ProtocolPackage = bincode::deserialize(&buffer[..])?;
        // println!("RECEIVED PACKAGE {:?}", received_message);
        Ok(received_message)
    }

    async fn send_package(&mut self, message: ProtocolPackage) -> StdResult<(), SendReceiveError> {
        let socket = self.get_server_socket();
        let serialized = bincode::serialize(&message)?;
        let len: u64 = serialized.len() as u64;
        socket.write_all(&len.to_le_bytes()).await?;
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
                    SendReceiveError::IoError(self).context(state)
                }
            }
        )*
    };
}

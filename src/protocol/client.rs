use std::sync::Arc;

use rust_state_machine::{AsyncProgress, ToStatesAndOutput, state_machine, with_context};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, mpsc};
use crate::{broker::BrokerError, impl_send_receive};

use crate::TcpStream;

use super::{Connection, FilteredTcpStream};
use super::{HasServerConnection, ProtocolPackage, SendReceiveError};


// ### STATES ###
#[derive(Clone, Debug)]
pub struct NotConnected;
#[derive(Debug)]
pub struct ServerConnected { server_socket: Connection }
#[derive(Debug)]
pub struct ServerConnectedAuthenticated { 
    server_socket: Connection, 
    username: String 
}

#[allow(dead_code)]
pub struct ServerChannelConnected { 
    server_socket: FilteredTcpStream,
    channel: String,
    username: String,
}

// TODO: replace with with ()
#[derive(Clone, Debug)]
pub struct Shared;
#[derive(Clone, Debug)]
pub enum Input {
    ConnectServer(String),
    Authenticate(String),
    GetChannelsList,
    ConnectChannel(String),
    SendMessage(String),
    DisconnectChannel,
    Disconnect,
}

#[derive(Debug)]
pub enum Reaction {
    LostConnection,
    IoError(std::io::Error),
    BinError(Box<bincode::ErrorKind>),
    MalformedPackage,
    ChannelList(Vec<String>),
    InvalidCommand,
    Success,
    Deny { error: BrokerError }
    // message from chat, disconnection notice, etc
}

state_machine! {
    pub
    async
    Name(ClientSideConnectionSM)
    Start(NotConnected)
    SharedContext(Shared)
    Input(Input)
    Output(Reaction)
    Edges {
        NotConnected => [NotConnected, ServerConnected],
        ServerConnected => [NotConnected, ServerConnected, ServerConnectedAuthenticated],
        ServerConnectedAuthenticated => [NotConnected, ServerConnectedAuthenticated, ServerChannelConnected],
        ServerChannelConnected => [NotConnected, ServerChannelConnected, ServerConnectedAuthenticated],
    }
}

impl_send_receive!(Reaction, NotConnected, NotConnected, ServerConnected, ServerConnectedAuthenticated, ServerChannelConnected);

impl From<SendReceiveError> for Reaction {
    fn from(error: SendReceiveError) -> Self {
        match error {
            SendReceiveError::IoError(error) => Reaction::IoError(error),
            SendReceiveError::BinError(error) => Reaction::BinError(error),
        }
    }
}


#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<NotConnectedEdges, ClientSideConnectionSM> for NotConnected {
    async fn transition(self, _shared: &mut Shared, input: Input) -> Option<(NotConnectedEdges, Reaction)> {
        let server_addres = match input {
            Input::ConnectServer(server_addres) => server_addres,
            _ => return Some((self.into(), Reaction::InvalidCommand)),
        };

        let server_connection = with_context!(TcpStream::connect(server_addres).await, self);
        let server_connection = Arc::new(Mutex::new(server_connection));
        let new_state = ServerConnected { server_socket: server_connection };
        Some((new_state.into(), Reaction::Success))
    } 
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ServerConnectedEdges, ClientSideConnectionSM> for ServerConnected {
    async fn transition(mut self, _shared: &mut Shared, input: Input) -> Option<(ServerConnectedEdges, Reaction)> {
        let new_username = match input {
            Input::Authenticate(username) => username,
            Input::Disconnect => return self.server_socket.disconnect().await,
            _ => return Some((self.into(), Reaction::InvalidCommand)),
        };
        let message = ProtocolPackage::ServerAuthenticationRequest{ username: new_username.clone() };
        // with_context!(self.)
        let response = with_context!(self.server_socket.send_package_and_receive(message).await, self);

        match response {
            ProtocolPackage::Accept => {
                let new_state = ServerConnectedAuthenticated { 
                    server_socket: self.server_socket, 
                    username: new_username 
                };
                Some((new_state.into(), Reaction::Success))
            },
            ProtocolPackage::Deny{ error } => Some((self.into(), Reaction::Deny{ error })),
            _ => Some((self.into(), Reaction::MalformedPackage))
        }
    } 
}

impl ServerConnectedAuthenticated {
    async fn connect_channel(mut self, channel: String) -> Option<(ServerConnectedAuthenticatedEdges, Reaction)> 
    {
        let message = ProtocolPackage::ChannelConnectionRequest{ channel: channel.clone() };
        let response = with_context!(self.server_socket.send_package_and_receive(message).await, self);
        match response {
            ProtocolPackage::Deny{ error } => Some((self.into(), Reaction::Deny{ error })),
            ProtocolPackage::Accept => {
                let (s1, r1) = mpsc::channel(1);
                let (s2, mut r2) = mpsc::channel(1);
                let mut socket_copy = self.server_socket.clone();
                // TODO:
                tokio::spawn(async move {
                    loop {
                        // tracing::debug!("1 IN FILTER THREAD");
                        let package = tokio::select! {
                            res = socket_copy.receive_package() => res,
                            _ = s1.closed() => break
                        };
                        // tracing::debug!("X IN FILTER THREAD");

                        if let Ok(new_package @ ProtocolPackage::ChatMessageReceive { .. }) = package {
                            // tracing::debug!("2 IN FILTER THREAD");
                            if s2.send(new_package).await.is_err() {
                                break;
                            }
                        } else {
                            // tracing::debug!("3 IN FILTER THREAD");
                            if s1.send(package).await.is_err() {
                                break;
                            }
                        }
                    }

                    std::mem::drop(s1);
                    std::mem::drop(s2);
                });
                
                tokio::spawn(async move {
                    while let Some(package) = r2.recv().await {
                        tracing::info!("RECEIVED MESSAGE: {:?}", package);
                    }
                });

                let filtered_tcp_stream = FilteredTcpStream {
                    socket: self.server_socket,
                    receiver: r1,
                };
                let new_state = ServerChannelConnected {
                    server_socket: filtered_tcp_stream,
                    username: self.username,
                    channel,
                };
                Some((new_state.into(), Reaction::Success))
            },
            _ => Some((self.into(), Reaction::MalformedPackage)),
        }
    }

    async fn list_channels(mut self)-> Option<(ServerConnectedAuthenticatedEdges, Reaction)>
    {
        let message = ProtocolPackage::InfoListChannelsRequest;
        let response = with_context!(self.server_socket.send_package_and_receive(message).await, self);
        match response {
            ProtocolPackage::Deny{ error } => Some((self.into(), Reaction::Deny{ error })),
            ProtocolPackage::InfoListChannelsReply{ channels } => Some((self.into(), Reaction::ChannelList(channels))),
            _ => Some((self.into(), Reaction::MalformedPackage)), // TODO: send malformed package notification back
        }
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ServerConnectedAuthenticatedEdges, ClientSideConnectionSM> for ServerConnectedAuthenticated {
    async fn transition(self, _shared: &mut Shared, input: Input) -> Option<(ServerConnectedAuthenticatedEdges, Reaction)> {
        match input {
            Input::ConnectChannel(channel) => self.connect_channel(channel).await,
            Input::GetChannelsList => self.list_channels().await,
            Input::Disconnect => self.server_socket.disconnect().await,
            _ => Some((self.into(), Reaction::InvalidCommand))
        }
    } 
}

impl ServerChannelConnected {
    async fn send_message(mut self, message: String) -> Option<(ServerChannelConnectedEdges, Reaction)>
    {
        let message = ProtocolPackage::ChatMessageSend { message };
        let reply = with_context!(self.server_socket.send_package_and_receive(message).await, self); 
        match reply {
            ProtocolPackage::Accept => Some((self.into(), Reaction::Success)),
            // ProtocolPackage::Deny { error } => Some((self.into(), error.into())),
            _ => Some((self.into(), Reaction::MalformedPackage))
        }
    }

    async fn disconnect_channel(mut self) -> Option<(ServerChannelConnectedEdges, Reaction)>
    {
        let message = ProtocolPackage::ChannelDisconnectNotification;
        let reply = with_context!(self.server_socket.send_package_and_receive(message).await, self); 
        match reply {
            ProtocolPackage::Accept => {
                let new_state = ServerConnectedAuthenticated {
                    server_socket: self.server_socket.get_unfiltered(),
                    username: self.username,
                };
                Some((new_state.into(), Reaction::Success)) 
            },
            // ProtocolPackage::Deny { error } => Some((self.into(), error.into())),
            _ => Some((self.into(), Reaction::MalformedPackage))
        }
    }
}


#[::rust_state_machine::async_trait::async_trait]
trait Disconnect<T: Send> {
    async fn disconnect(mut self) -> Option<(T, Reaction)>;
} 

#[::rust_state_machine::async_trait::async_trait]
impl<D: From<NotConnected> + Send> Disconnect<D> for Connection 
{
    async fn disconnect(mut self) -> Option<(D, Reaction)> {
        let message = ProtocolPackage::DisconnectNotification;
        match self.send_package(message).await {
            Ok(_) => {
                match self.lock_owned().await.shutdown().await {
                    Ok(_) => Some((NotConnected.into(), Reaction::Success)),
                    Err(error) => Some((NotConnected.into(), Reaction::IoError(error)))
                }
            },
            Err(error) => Some((NotConnected.into(), error.into())),
        }
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl<D: From<NotConnected> + Send> Disconnect<D> for FilteredTcpStream {
    async fn disconnect(mut self) -> Option<(D, Reaction)> {
        self.get_unfiltered().disconnect().await
    }
} 

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ServerChannelConnectedEdges, ClientSideConnectionSM> for ServerChannelConnected {
    async fn transition(self, _shared: &mut Shared, input: Input) -> Option<(ServerChannelConnectedEdges, Reaction)> {
        match input {
            Input::SendMessage(message) => self.send_message(message).await,
            Input::DisconnectChannel => self.disconnect_channel().await,
            Input::Disconnect => {
                self.server_socket.disconnect().await
            },
            _ => Some((self.into(), Reaction::InvalidCommand))
        }
    } 
}

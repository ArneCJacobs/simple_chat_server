use futures::Stream;
use rust_state_machine::{AsyncProgress, ToStatesAndOutput, state_machine, with_context};
use smol::{net::{TcpStream, TcpListener}, io::AsyncWriteExt, channel::Receiver, stream::StreamExt};
use smol::channel;

use crate::{broker::BrokerError, impl_send_receive};

use super::{HasServerConnection, ProtocolPackage, SendReceiveError, protocol_package_stream::{self, print_type_of, to_protocolpackage_stream}};


// ### STATES ###
#[derive(Clone, Debug)]
pub struct NotConnected;
#[derive(Clone, Debug)]
pub struct ServerConnected { server_socket: TcpStream }
#[derive(Clone, Debug)]
pub struct ServerConnectedAuthenticated { server_socket: TcpStream, username: String }
pub struct ServerChannelConnected { 
    connection: TcpStream,
    packages: Receiver<Result<ProtocolPackage, SendReceiveError>>,
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
        let response = with_context!(self.server_socket.send_package_and_receive(message).await, self);

        match response {
            ProtocolPackage::Accept => {
                let new_state = ServerConnectedAuthenticated { server_socket: self.server_socket, username: new_username };
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
                // TODO: make a filtered channel where all protocol messages which indicate a
                // received chat message are handeled seperately
                let (s1, r1) = channel::unbounded();
                let (s2, r2) = channel::unbounded();
                let stream = to_protocolpackage_stream(self.server_socket.clone());
                let mut stream = Box::pin(stream);
                let _ = smol::spawn(async move {
                    tracing::debug!("FUCKING YEET");
                    while !s1.is_closed() {
                        let package = stream.next().await;
                        if package.is_none() {
                            break;
                        }
                        let package = package.unwrap();
                        if let Ok(new_package @ ProtocolPackage::ChatMessageReceive { .. }) = package {
                            s2.send(new_package).await.unwrap();
                        } else {
                            s1.send(package).await.unwrap();
                        }
                    }
                    s1.close();
                    s2.close();
                });

                let _ = smol::spawn(async move {
                    while !r2.is_closed() {
                        let package = r2.recv().await;
                        tracing::debug!("RECEIVED MESSAGE: {:?}", package);
                    }
                });
                // let filtered_stream = stream.lef
                let new_state = ServerChannelConnected {
                    connection: self.server_socket,
                    packages: r1,
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
            _ => Some((self.into(), Reaction::MalformedPackage)), // TODO: send malformed package
            // notification back
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
        with_context!(self.connection.send_package(message).await, self);

        // TODO: no unwrap here
        let reply = with_context!(self.packages.recv().await.unwrap(), self); 
        match reply {
            ProtocolPackage::Accept => Some((self.into(), Reaction::Success)),
            // ProtocolPackage::Deny { error } => Some((self.into(), error.into())),
            _ => Some((self.into(), Reaction::MalformedPackage))
        }
    }

    async fn disconnect_channel(mut self) -> Option<(ServerChannelConnectedEdges, Reaction)>
    {
        let message = ProtocolPackage::ChannelDisconnectNotification;
        with_context!(self.connection.send_package(message).await, self);

        // TODO: no unwrap here
        let reply = with_context!(self.packages.recv().await.unwrap(), self); 
        match reply {
            ProtocolPackage::Accept => {
                self.packages.close();
                let new_state = ServerConnectedAuthenticated {
                    server_socket: self.connection,
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
impl<T:HasServerConnection + Send, D: From<NotConnected> + Send> Disconnect<D> for T {
    async fn disconnect(mut self) -> Option<(D, Reaction)> {
        let message = ProtocolPackage::DisconnectNotification;
        match self.get_server_socket().send_package(message).await {
            Ok(_) => {
                match self.get_server_socket().close().await {
                    Ok(_) => Some((NotConnected.into(), Reaction::Success)),
                    Err(error) => Some((NotConnected.into(), Reaction::IoError(error)))
                }
            },
            Err(error) => Some((NotConnected.into(), error.into())),
        }
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ServerChannelConnectedEdges, ClientSideConnectionSM> for ServerChannelConnected {
    async fn transition(self, _shared: &mut Shared, input: Input) -> Option<(ServerChannelConnectedEdges, Reaction)> {
        match input {
            Input::SendMessage(message) => self.send_message(message).await,
            Input::DisconnectChannel => self.disconnect_channel().await,
            Input::Disconnect => {
                self.packages.close();
                self.connection.disconnect().await
            },
            _ => Some((self.into(), Reaction::InvalidCommand))
        }
    } 
}

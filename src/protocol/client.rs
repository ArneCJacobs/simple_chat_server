use rust_state_machine::{AsyncProgress, ToStatesAndOutput, state_machine, with_context};
use smol::net::{TcpStream, TcpListener};

use crate::{broker::ErrorType, impl_send_receive};

use super::{HasServerConnection, ProtocolPackage, SendReceiveError};


// ### STATES ###
#[derive(Clone, Debug)]
pub struct NotConnected;
#[derive(Clone, Debug)]
pub struct ServerConnected { server_socket: TcpStream }
#[derive(Clone, Debug)]
pub struct ServerConnectedAuthenticated { server_socket: TcpStream, username: String }
#[derive(Clone, Debug)]
pub struct ServerChannelConnected { 
    server_socket: TcpStream,
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
    Deny { error: ErrorType }
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

// impl ToStatesAndOutput<NotConnected, NotConnectedEdges, Reaction> for std::io::Error {
//     fn context(self, state: NotConnected) -> (NotConnectedEdges, Reaction) {
//        (state.into(), Reaction::IoError(self))  
//     }
// }
//


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

        println!("HERE");

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
                let new_state = ServerChannelConnected {
                    server_socket: self.server_socket,
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
            _ => Some((self.into(), Reaction::MalformedPackage)),
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
        todo!();
    }

    async fn disconnect_channel(mut self) -> Option<(ServerChannelConnectedEdges, Reaction)>
    {
        todo!();
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
            Ok(_) => Some((NotConnected.into(), Reaction::Success)),
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
            Input::Disconnect => self.server_socket.disconnect().await,
            _ => Some((self.into(), Reaction::MalformedPackage))
        }
    } 
}



// ### EDGES ###
// #[derive(Debug, Clone)]
// pub enum NotConnectedEdges {
//     Connect(ClientSideConnectionFMS<ServerConnected>),
// }
//
// #[derive(Debug, Clone)]
// pub enum ServerConnectedEdges {
//     Disconnected,
//     Authenticated(ClientSideConnectionFMS<ServerConnectedAuthenticated>),
// }

// ### FSM ###

// #[derive(Debug, Clone)]
// pub struct ClientSideConnectionFMS<S: Clone> {
//     state: S,
// }

// impl ClientSideConnectionFMS<NotConnected> {
//     pub fn new() -> Self {
//         ClientSideConnectionFMS { state: NotConnected }
//     }
//
//     pub async fn connect(addr: String) -> CResult<ServerConnected, NotConnected> {
//         let tcp_connection = TcpStream::connect(addr).await?;
//         Ok(ClientSideConnectionFMS {
//             state: ServerConnected{ server_socket: tcp_connection }
//         })
//     }
// }
//
// impl ClientSideConnectionFMS<ServerConnected> {
//     pub async fn authenticate(mut self, username: String) -> CResult<ServerConnectedAuthenticated, ServerConnected> {
//         use ProtocolPackage::*;
//         let message = ServerConnectionRequest { username };
//         let reply: ClientResult<ProtocolPackage, ServerConnected> = self.state.server_socket.send_package_and_receive(message).await;
//         let reply = reply?;
//
//         match reply {
//             ServerConnectedAuthenticated => Ok(),
//             ServerConnect
//         }
//     }
// }


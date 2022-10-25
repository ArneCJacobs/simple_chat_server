use rust_state_machine::{AsyncProgress, ToStatesAndOutput, state_machine, with_context};
use smol::net::{TcpStream, TcpListener};

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
// TODO: replace with both of these with ()
pub struct Shared;
pub enum Input {
    ConnectServer(String),
    Authenticate(String),
}

pub enum Reaction {
    IoError(std::io::Error),
    BinError(Box<bincode::ErrorKind>),
    MalformedPackage,
    InvalidCommand,
    Success,
    Deny { message: String }
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

impl ToStatesAndOutput<NotConnected, NotConnectedEdges, Reaction> for std::io::Error {
    fn context(self, state: NotConnected) -> (NotConnectedEdges, Reaction) {
       (state.into(), Reaction::IoError(self))  
    }
}

impl ToStatesAndOutput<ServerConnected, ServerConnectedEdges, Reaction> for SendReceiveError {
    fn context(self, state: ServerConnected) -> (ServerConnectedEdges, Reaction) {
        let reaction = match self {
            SendReceiveError::IoError(error) => Reaction::IoError(error),
            SendReceiveError::BinError(error) => Reaction::BinError(error),
        };
        (state.into(), reaction)
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
            _ => return Some((self.into(), Reaction::InvalidCommand)),
        };
        let message = ProtocolPackage::ServerAuthenticationRequest{ username: new_username.clone() };
        let response = with_context!(self.server_socket.send_package_and_receive(message).await, self);

        match response {
            ProtocolPackage::Accept => {
                let new_state = ServerConnectedAuthenticated { server_socket: self.server_socket, username: new_username };
                Some((new_state.into(), Reaction::Success))
            },
            ProtocolPackage::Deny{ message } => Some((self.into(), Reaction::Deny{ message })),
            _ => Some((self.into(), Reaction::MalformedPackage))
        }
    } 
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ServerConnectedAuthenticatedEdges, ClientSideConnectionSM> for ServerConnectedAuthenticated {
    async fn transition(self, _shared: &mut Shared, _input: Input) -> Option<(ServerConnectedAuthenticatedEdges, Reaction)> {
        None
    } 
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ServerChannelConnectedEdges, ClientSideConnectionSM> for ServerChannelConnected {
    async fn transition(self, _shared: &mut Shared, _input: Input) -> Option<(ServerChannelConnectedEdges, Reaction)> {
        None
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


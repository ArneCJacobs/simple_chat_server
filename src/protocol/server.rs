use std::{sync::Arc, fmt::Debug};

use rust_state_machine::{AsyncProgress, state_machine, with_context};
use smol::{net::TcpStream, lock::Mutex};

use crate::{broker::Broker, impl_send_receive};

use super::{HasServerConnection, ProtocolPackage, SendReceiveError};


// ### STATES ###
#[derive(Debug, Clone)]
pub struct ClientConnection{ 
    pub socket: TcpStream 
}
#[derive(Debug, Clone)]
pub struct ClientConnectionAuthenticated{ 
    socket: TcpStream, 
    username: String 
}
#[derive(Debug, Clone)]
pub struct ClientChannelConnection{ 
    socket: TcpStream, 
    username: String, 
    channel: String 
}
pub struct Disconnected;
pub struct SharedContext{ 
    pub broker: Arc<Mutex<Broker>> 
} 

#[derive(Debug)]
pub enum Reaction {
    Success,
    Disconnected,
    LostConnecion,
    MalformedPackage,
    UsernameAlreadyTaken,
    IoError(std::io::Error),
    BinError(Box<bincode::ErrorKind>),
}

state_machine!{
    pub
    async
    Name(ServerSideConnectionSM)
    Start(ClientConnection)
    SharedContext(SharedContext)
    Input(ProtocolPackage)
    Output(Reaction)
    Edges {
        ClientConnection => [Disconnected, ClientConnection, ClientConnectionAuthenticated],
        ClientConnectionAuthenticated => [Disconnected, ClientConnectionAuthenticated, ClientChannelConnection],
        ClientChannelConnection => [Disconnected, ClientChannelConnection, ClientConnectionAuthenticated],
    }
}

impl From<SendReceiveError> for Reaction {
    fn from(error: SendReceiveError) -> Self {
        match error {
            SendReceiveError::IoError(error) => Reaction::IoError(error),
            SendReceiveError::BinError(error) => Reaction::BinError(error),
        }
    }
}

impl_send_receive!(Reaction, Disconnected, Disconnected, ClientConnection, ClientConnectionAuthenticated, ClientChannelConnection);

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ClientConnectionEdges, ServerSideConnectionSM> for ClientConnection {
   async fn transition(mut self, shared: &mut SharedContext, input: ProtocolPackage) -> Option<(ClientConnectionEdges, Reaction)> {
        let username = match input {
            ProtocolPackage::ServerAuthenticationRequest{ username } => username,
            ProtocolPackage::DisconnectNotification => return Some((Disconnected.into(), Reaction::Disconnected)),
            _ => return Some((self.into(), Reaction::MalformedPackage)),
        };

        let mut guard = shared.broker.lock_arc().await;               
        let res = guard.register_username(username.clone());
        std::mem::drop(guard); // not strictly needed but the faster the mutex guard is dropped the better 
        if let Err(error) = res {
            let message = ProtocolPackage::Deny{ error };
            with_context!(self.socket.send_package(message).await, self);
            return Some((self.into(), Reaction::UsernameAlreadyTaken));
        } else {
            let reply = ProtocolPackage::Accept;
            with_context!(self.socket.send_package(reply).await, self);
            let new_state = ClientConnectionAuthenticated {
                socket: self.socket,
                username,
            };
            return Some((new_state.into(), Reaction::Success));
        }
   } 
}
impl ClientConnectionAuthenticated {
    async fn list_channels(mut self, shared: &mut SharedContext) -> Option<(ClientConnectionAuthenticatedEdges, Reaction)>
    {
        let guard = shared.broker.lock_arc().await;
        let channels = guard.get_channels();
        std::mem::drop(guard);
        let message = ProtocolPackage::InfoListChannelsReply{ channels };
        with_context!(self.socket.send_package(message).await, self);
        Some((self.into(), Reaction::Success))
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ClientConnectionAuthenticatedEdges, ServerSideConnectionSM> for ClientConnectionAuthenticated {
    async fn transition(self, shared: &mut SharedContext, input: ProtocolPackage) -> Option<(ClientConnectionAuthenticatedEdges, Reaction)> {
        match input {
            ProtocolPackage::InfoListChannelsRequest => self.list_channels(shared).await,
            ProtocolPackage::DisconnectNotification => return Some((Disconnected.into(), Reaction::Disconnected)),
            _ => return Some((self.into(), Reaction::MalformedPackage)),
        }
    } 
}


#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ClientChannelConnectionEdges, ServerSideConnectionSM> for ClientChannelConnection {
    async fn transition(self, shared: &mut SharedContext, input: ProtocolPackage) -> Option<(ClientChannelConnectionEdges, Reaction)> {
        todo!();  
    } 
}


// ### EDGES ###
// #[derive(Debug, Clone)]
// pub enum ClientConnectionEdges<'a> {
//     Disconnected,
//     Authenticated(ServerSideConnectionFMS<'a, ClientConnectionAuthenticated>),
// }
//
// #[derive(Debug, Clone)]
// pub enum ClientConnectionAuthenticatedEdges<'a> {
//     Disconnected,
//     ChannelConnect(ServerSideConnectionFMS<'a, ClientChannelConnection>),
//     ListChannels(ServerSideConnectionFMS<'a, ClientConnectionAuthenticated>),
// }
//
// #[derive(Debug, Clone)]
// pub struct ServerSideConnectionFMS<'a, S: Clone> {
//     broker: &'a Arc<Mutex<Broker>>,
//     state: S,
// }
//
// impl<'a> ServerSideConnectionFMS<'a, ClientConnection> {
//     pub fn new(broker: &'a Arc<Mutex<Broker>>, connection: TcpStream) -> Self {
//         ServerSideConnectionFMS {
//             broker,
//             state: ClientConnection {
//                 socket: connection
//             }
//         }
//     }
//
//     pub async fn authenticate(mut self, username: String) -> SResult<'a, ClientConnectionAuthenticated, ClientConnection> {
//         let mut guard = self.broker.lock_arc().await;               
//         guard.register_username(username.clone())
//             .map_err(|e| {
//                 ServerFailEdges::from_errortype(self.clone(), e)
//             })?;
//
//         // TODO remove clone, self should be able to be given as value, 
//         //but rust cannot infer that self won't be used after this statement
//
//         std::mem::drop(guard); // not strictly needed but the faster the mutex guard is dropped the better 
//
//         let reply = ProtocolPackage::ServerConnectionAccept;
//         let temp: ServerResult<'a, (), ClientConnection> = self.state.socket.send_package(reply).await;
//         temp?;
//
//         Ok(ServerSideConnectionFMS {
//             broker: self.broker,
//             state: ClientConnectionAuthenticated {
//                 socket: self.state.socket,
//                 username,
//             }
//         })
//     }
//
//     pub async fn disconnect(self) -> ServerResult<'a, (), ClientConnection> {
//         self.state.socket.shutdown(std::net::Shutdown::Both)?;
//         Ok(())
//     }
//
//     pub async fn listen(mut self) -> ServerResult<'a, ClientConnectionEdges<'a>, ClientConnection> {
//         let package: ServerResult<'a, ProtocolPackage, ClientConnection> = self.state.socket.receive_package().await;
//         let package = package?;
//         use crate::protocol::ProtocolPackage::*;
//         use ClientConnectionEdges::*;
//         match package {
//             ServerConnectionRequest { username } => Ok(Authenticated(self.authenticate(username).await?)), 
//             DisconnectNotification => {
//                 self.disconnect().await?;
//                 Ok(Disconnected)
//             }, 
//             _ => Err(ServerFailEdges::MalformedPackage(self)) 
//         }
//     }
// }
//
//
// impl<'a> ServerSideConnectionFMS<'a, ClientConnectionAuthenticated> {
//     pub async fn disconnect(self) -> ServerResult<'a, (), ClientConnectionAuthenticated> {
//         let mut guard = self.broker.lock_arc().await;               
//         guard.deregister_username(&self.state.username);
//         self.state.socket.shutdown(std::net::Shutdown::Both)?;
//         Ok(())
//     }
// }
//
// impl<'a, E, T: HasServerConnection<E> + Clone> HasServerConnection<E> for ServerSideConnectionFMS<'a, T> 
//     where E: From<Box<bincode::ErrorKind>> + From<std::io::Error> {
//     fn get_server_socket(&mut self) ->  &mut TcpStream {
//         self.state.get_server_socket()
//     }
// }


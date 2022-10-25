use std::{sync::Arc, fmt::Debug};

use smol::{net::TcpStream, lock::Mutex};

use crate::broker::Broker;

use super::{HasServerConnection, ProtocolPackage};


// ### STATES ###
#[derive(Debug, Clone)]
pub struct ClientConnection{ socket: TcpStream }
#[derive(Debug, Clone)]
pub struct ClientConnectionAuthenticated{ socket: TcpStream, username: String }
#[derive(Debug, Clone)]
pub struct ClientChannelConnection{ socket: TcpStream, username: String, channel: String }

impl<E> HasServerConnection<E> for ClientConnection
    where E: From<Box<bincode::ErrorKind>> + From<std::io::Error> {

    fn get_server_socket(&mut self) ->  &mut TcpStream {
        &mut self.socket
    }
}

impl<E> HasServerConnection<E> for ClientConnectionAuthenticated
    where E: From<Box<bincode::ErrorKind>> + From<std::io::Error> {

    fn get_server_socket(&mut self) ->  &mut TcpStream {
        &mut self.socket
    }
}

impl<E> HasServerConnection<E> for ClientChannelConnection
    where E: From<Box<bincode::ErrorKind>> + From<std::io::Error> {

    fn get_server_socket(&mut self) ->  &mut TcpStream {
        &mut self.socket
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


use smol::net::{TcpStream, TcpListener};

use super::{HasServerConnection, ProtocolPackage};


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


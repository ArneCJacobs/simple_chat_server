use smol::net::TcpStream;

use crate::error::Result;
use crate::protocol::ProtocolPackage;
use crate::protocol::HasServerConnection;


pub struct NotConnected;
pub struct ServerConnected { server_socket: TcpStream }
pub struct ChannelConnected { 
    server_socket: TcpStream,
    channel: String,
}

// TODO
// impl HasServerConnection for ServerConnected {
//     fn get_server_socket(&mut self) -> &mut TcpStream {
//         &mut self.server_socket
//     }
// }
//
// impl HasServerConnection for ChannelConnected {
//     fn get_server_socket(&mut self) -> &mut TcpStream {
//         &mut self.server_socket
//     }
// }
//
//
// pub struct ClientSideConnectionFSM<S> {
//     state: S,
// }
//
// impl ClientSideConnectionFSM<NotConnected> {
//     pub fn new() -> Self {
//         ClientSideConnectionFSM {
//             state: NotConnected
//         }
//     }
//
//     pub async fn connect(self, server_addr: String) -> Result<ClientSideConnectionFSM<ServerConnected>> {
//         Ok(ClientSideConnectionFSM {
//             state: ServerConnected{ 
//                 server_socket: TcpStream::connect(server_addr).await? 
//             },
//         })
//
//     }
// }
//
// impl ClientSideConnectionFSM<ServerConnected> {
//     // TODO return an error type here which contains a different state of the state machine and the error that caused it.
//     pub async fn join_channel(mut self, channel: String) -> Result<ClientSideConnectionFSM<ChannelConnected>> {
//         let received_message = self.state.send_package_and_receive(ChannelConnectionRequest { channel: channel.clone() }).await?;
//
//         use ProtocolPackage::*;
//         match received_message {
//             ChannelConnectionRequestAccept => {
//                 Ok(ClientSideConnectionFSM { 
//                     state: ChannelConnected { 
//                         server_socket: self.state.server_socket,
//                         channel
//                     } 
//                 })
//             },
//             ChannelConnectionRequestDeny => {
//                 Err("The channel connection was refused")?
//             }
//             _ => {
//
//                 Err("Unexpected package type received")?
//             }
//         }
//     }
// }
//
// // TODO
// pub enum ClientConnectionError {
//     ProcessError,
//     ChannelConnectionRequestDeny(ClientSideConnectionFSM<ServerConnected>),
//     MalformedMessage(ClientSideConnectionFSM<ChannelConnected>),
//     ConnectionLost(ClientSideConnectionFSM<NotConnected>), // TODO add error type here
// }
//
// impl ClientSideConnectionFSM<ChannelConnected> {
//     pub async fn send_message(mut self, message: String) -> Result<ClientSideConnectionFSM<ChannelConnected>> {
//         // TODO factor out code
//         let message = ProtocolPackage::ChatMessageSend { message };
//         let received_message = self.state.send_package_and_receive(message).await?;
//
//         use ProtocolPackage::*;
//         match received_message {
//             ChatMessageSendAccept => {Ok(self)},
//             ChatMessageSendDeny { error } => {Err(error)? },
//             _ => {Err("Unexpected package type received")? }
//         }
//         
//     }
// }

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

impl HasServerConnection for ServerConnected {
    fn get_server_socket(&mut self) -> &mut TcpStream {
        &mut self.server_socket
    }
}

impl HasServerConnection for ChannelConnected {
    fn get_server_socket(&mut self) -> &mut TcpStream {
        &mut self.server_socket
    }
}


pub struct ClientConnectionFSM<S> {
    state: S,
}

impl ClientConnectionFSM<NotConnected> {
    pub fn new() -> Self {
        ClientConnectionFSM {
            state: NotConnected
        }
    }

    pub async fn connect(self, server_addr: String) -> Result<ClientConnectionFSM<ServerConnected>> {
        Ok(ClientConnectionFSM {
            state: ServerConnected{ 
                server_socket: TcpStream::connect(server_addr).await? 
            },
        })

    }
}

impl ClientConnectionFSM<ServerConnected> {
    // TODO return an error type here which contains a different state of the state machine and the error that caused it.
    pub async fn join_channel(mut self, channel: String) -> Result<ClientConnectionFSM<ChannelConnected>> {
        let received_message = self.state.send_package_and_receive(ChannelConnectionRequest { channel: channel.clone() }).await?;

        use ProtocolPackage::*;
        match received_message {
            ChannelConnectionRequestAccept => {
                Ok(ClientConnectionFSM { 
                    state: ChannelConnected { 
                        server_socket: self.state.server_socket,
                        channel
                    } 
                })
            },
            ChannelConnectionRequestDeny => {
                Err("The channel connection was refused")?
            }
            _ => {

                Err("Unexpected package type received")?
            }
        }
    }
}

// TODO
pub enum ClientConnectionError {
    ProcessError,
    ChannelConnectionRequestDeny(ClientConnectionFSM<ServerConnected>),
    MalformedMessage(ClientConnectionFSM<ChannelConnected>),
    ConnectionLost(ClientConnectionFSM<NotConnected>), // TODO add error type here
}

impl ClientConnectionFSM<ChannelConnected> {
    pub async fn send_message(mut self, message: String) -> Result<ClientConnectionFSM<ChannelConnected>> {
        // TODO factor out code
        let message = ProtocolPackage::ChatMessageSend { message };
        let received_message = self.state.send_package_and_receive(message).await?;

        use ProtocolPackage::*;
        match received_message {
            ChatMessageSendAccept => {Ok(self)},
            ChatMessageSendDeny { error } => {Err(error)? },
            _ => {Err("Unexpected package type received")? }
        }
        
    }
}

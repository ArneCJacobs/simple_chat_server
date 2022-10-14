use std::{collections::HashMap, sync::Arc, fmt::{Debug, Display, write}};

use smol::{net::TcpStream, lock::Mutex};
use std::io;

use crate::{error::Result, broker::Broker};
use std::error::Error;

use super::HasServerConnection;

pub struct ClientConnection{ socket: TcpStream }
pub struct ClientConnectionAuthenticated{ socket: TcpStream, username: String }
pub struct ClientChannelConnection{ socket: TcpStream, username: String, channel: String }

pub enum ClientConnectionEdges<'a> {
    Disconnected,
    Authenticated(ServerSideConnectionFMS<'a, ClientConnectionAuthenticated>),
}

pub enum ClientConnectionAuthenticatedEdges<'a> {
    Disconnected,
    ChannelConnect(ServerSideConnectionFMS<'a, ClientChannelConnection>),
    ListChannels(ServerSideConnectionFMS<'a, ClientConnectionAuthenticated>),
}

#[derive(Debug)]
pub struct ServerSideConnectionFMS<'a, S> {
    broker: &'a Arc<Mutex<Broker>>,
    state: S,
}


#[derive(Debug)]
pub enum FailEdges<'a, T: Debug, E: Error> {
    Disconnected,
    Rejected(ServerSideConnectionFMS<'a, T>, E),
    MalformedPackage(ServerSideConnectionFMS<'a, T>),
}

impl<'a, T: Debug, E: Error> Display for FailEdges<'a, T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Disconnected =>  write!(f, "Tcp connection closed"),
            Self::Rejected(_, error) => write!(f, "Rejected due to following error: {:?}", error),
            Self::MalformedPackage(_) => write!(f, "Malformed/Unexpected package received")
        }
    }
}

impl<'a, T: Debug, E: Error + 'static> Error for FailEdges<'a, T, E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            Self::Disconnected => None,
            Self::MalformedPackage(_) => None,
            Self::Rejected(_,ref error) => Some(error),
        }
    }
}


// TODO create From<_>

impl<'a> ServerSideConnectionFMS<'a, ClientConnection> {
    pub fn new(broker: &'a Arc<Mutex<Broker>>, connection: TcpStream) -> Self {
        ServerSideConnectionFMS {
            broker,
            state: ClientConnection {
                socket: connection
            }
        }
    }

    pub async fn authenticate(mut self) -> Result<ServerSideConnectionFMS<'a, ClientConnectionAuthenticated>> {
        use crate::protocol::ProtocolPackage::*;
        let package = self.state.socket.receive_package().await?;
        let username = match package {
            ServerConnectionRequest { username } =>  Ok(username),
            _ => Err(io::Error::new(io::ErrorKind::Other, "Malformed message"))
        }?;

        let mut guard = self.broker.lock_arc().await;               
        guard.register_username(username.clone())?;
        std::mem::drop(guard); // not strictly needed but the faster the mutex guard is dropped the better 

        let reply = ServerConnectionAccept;
        self.state.socket.send_package(reply).await?;

        Ok(ServerSideConnectionFMS {
            broker: self.broker,
            state: ClientConnectionAuthenticated {
                socket: self.state.socket,
                username,
            }
        })
    }
}


impl<'a> ServerSideConnectionFMS<'a, ClientConnectionAuthenticated> {
    pub async fn disconnect(self) {
        let mut guard = self.broker.lock_arc().await;               
        guard.deregister_username(&self.state.username);
    }

    pub async fn connect_channel(mut self) -> Result<ServerSideConnectionFMS<'a, ClientChannelConnection>> {
        
    }
}


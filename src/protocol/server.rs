use std::{io, sync::Arc, fmt::Debug};

use smol::{net::TcpStream, lock::Mutex};

use crate::{broker::Broker, error::{SResult, FailEdges, Result}};

use super::HasServerConnection;

#[derive(Debug, Clone)]
pub struct ClientConnection{ socket: TcpStream }
#[derive(Debug, Clone)]
pub struct ClientConnectionAuthenticated{ socket: TcpStream, username: String }
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct ServerSideConnectionFMS<'a, S: Clone> {
    broker: &'a Arc<Mutex<Broker>>,
    state: S,
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

    pub async fn authenticate(mut self) -> SResult<'a, ClientConnectionAuthenticated, ClientConnection> {
        use crate::protocol::ProtocolPackage::*;
        let package = self.state.socket.receive_package().await?;
        let username = match package {
            ServerConnectionRequest { username } =>  Ok(username),
            _ => Err(FailEdges::MalformedPackage(self.clone()))
        }?;

        let mut guard = self.broker.lock_arc().await;               
        guard.register_username(username.clone()).map_err(|e| FailEdges::from_errortype(self.clone(), e))?;
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
}


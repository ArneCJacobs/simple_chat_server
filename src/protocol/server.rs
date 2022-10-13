use std::collections::HashMap;

use smol::net::TcpStream;

use crate::error::Result;

use super::HasServerConnection;


struct ClientConnection{ socket: TcpStream, username: String }
struct ClientChannelConnection;


struct ClientConnectionData {
    username: String,
    connection: TcpStream,
}

impl HasServerConnection for ClientConnection {
    fn get_server_socket(&mut self) -> &mut TcpStream {
        &mut self.socket
    }
}

struct ServerConnectionFMS<S> {
    channels: HashMap<String, Vec<ClientConnectionData>>,
    state: S,
}

impl ServerConnectionFMS<ClientConnection> {
    fn new(connection: TcpStream, username: String) -> Result<ServerConnectionFMS<ClientConnection>> {
        let mut map = HashMap::new();
        map.insert("welcome".to_string(), Vec::new());
        Ok(ServerConnectionFMS {
            channels: map,
            state: ClientConnection {
                username,
                socket: connection
            }
        })
    }

    fn connect_channel(mut self, channel: String) -> Result<ServerConnectionFMS<ClientChannelConnection>> {
        if !self.channels.contains_key(&channel) {
            self.channels.insert(channel.clone(), Vec::new());
        }
        let channel_connection = self.channels.get_mut(&channel).ok_or("should never be called")?;

        let client_connection_data = ClientConnectionData {
            username: self.state.username,
            connection: self.state.socket,
        };

        channel_connection.push(client_connection_data);

        Ok(ServerConnectionFMS{ 
            channels: self.channels, 
            state: ClientChannelConnection 
        })
    } 


    // TODO use async to wait for any input on any channel
}


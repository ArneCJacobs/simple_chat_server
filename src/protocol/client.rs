use smol::net::TcpStream;

pub struct NotConnected;
pub struct ServerConnected { server_socket: TcpStream }
pub struct ChannelConnected { 
    server_socket: TcpStream,
    channel: String,
}

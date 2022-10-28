
use smol::{net::TcpStream, stream::Stream};
use super::{ProtocolPackage, HasServerConnection};

pub async fn to_protocolpackage_stream(stream: TcpStream) -> impl Stream<Item=ProtocolPackage> {
    smol::stream::unfold(stream, |mut stream| async move {
        if let Ok(package) = stream.receive_package().await {
            return Some((package, stream));
        } 
        None
    })
}

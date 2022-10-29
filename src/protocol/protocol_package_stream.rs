
use smol::{net::TcpStream, stream::Stream};
use super::{ProtocolPackage, HasServerConnection, SendReceiveError};

pub type Temp = Result<ProtocolPackage, SendReceiveError>;
pub fn to_protocolpackage_stream(stream: TcpStream) -> 
impl Stream<Item=Temp> 
{
    smol::stream::unfold(stream, |mut stream| async move {
        let res = stream.receive_package().await;
        match res {
            Ok(package) => Some((Ok(package), stream)),
            Err(error) => Some((Err(error), stream)),
            _ => None
        } 
    })
}

pub fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>());
}

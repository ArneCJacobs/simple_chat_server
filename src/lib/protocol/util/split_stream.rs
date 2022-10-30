use async_trait::async_trait;
use tokio::{task::JoinHandle, net::{tcp::{OwnedWriteHalf, OwnedReadHalf}, TcpStream}, sync::{mpsc::{Sender, self}, oneshot}};
use std::result::Result as StdResult;

use crate::protocol::{ProtocolPackageSender, ProtocolPackage, SendReceiveError, ProtocolPackageReader, HasServerConnection};

struct SplitStream {
    handle: JoinHandle<OwnedWriteHalf>,
    reader: OwnedReadHalf,
    sender: Sender<ProtocolPackage>,
    kill_sender: Sender<()>,
}

impl SplitStream {
    async fn new(stream: TcpStream) -> Self {
        let (reader, mut writer) = stream.into_split();
        let (sender, mut receiver) = mpsc::channel(10);
        let (kill_sender, mut kill_receiver) = mpsc::channel::<()>(1);
        let handle = tokio::spawn(async move {
            loop {
                let package = receiver.recv().await;
                let package = tokio::select! {
                    val = receiver.recv() => val,
                    _ = kill_receiver.recv() => {
                        receiver.close(); // close the channel without
                        // dropping it so the remaining messages can be drained
                        continue;
                    }
                };
                if package.is_none() {
                    break;
                }
                let package = package.unwrap();
                let res = writer.send_package(package).await;

                //TODO: handle error
                res.unwrap();

            }
            writer
        });
        
        SplitStream {
            reader,
            handle,
            sender,
            kill_sender,
        }
    }

    fn get_sender_clone(&self) -> Sender<ProtocolPackage> {
        self.sender.clone()
    }

    async fn unsplit(mut self) -> TcpStream {
        self.kill_sender.send(()).await.unwrap();
        let writer = self.handle.await.unwrap();
        self.reader.reunite(writer).unwrap()
    }
}

#[async_trait]
impl ProtocolPackageSender for SplitStream {
    async fn send_package(&mut self, message: ProtocolPackage) -> StdResult<(), SendReceiveError> {
        if self.sender.send(message).await.is_err() {
            return Err(SendReceiveError::IoError(
                std::io::Error::new(
                    std::io::ErrorKind::NotConnected, 
                    "Connection lost"
                )
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl ProtocolPackageReader for SplitStream {
    async fn receive_package(&mut self) -> StdResult<ProtocolPackage, SendReceiveError> {
        self.reader.receive_package().await
    }
}

impl HasServerConnection for SplitStream {}


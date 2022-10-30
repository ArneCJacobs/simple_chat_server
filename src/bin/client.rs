use std::{time::Duration, env};
use std::error::Error;

use tracing::Level;

use simple_chat_protocol::protocol::client::{ClientSideConnectionSM, Input, NotConnected, Shared};
use rust_state_machine::{StateMachineAsync, StatefulAsyncStateMachine};

const ADDR: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .without_time()
        .with_target(false)
        .finish();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;

    let args: Vec<String> = env::args().collect();
    tokio::time::sleep(Duration::from_secs_f64(0.1)).await;
    tracing::info!("Streaming to {} as {}", ADDR, args[1]);
    let shared = Shared;
    let start_state = NotConnected;
    let mut client: StateMachineAsync<ClientSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
    let commands = vec![
        Input::ConnectServer(ADDR.to_string()),
        Input::Authenticate(args[1].clone()),
        Input::GetChannelsList,
        Input::ConnectChannel("Welcome".to_string()),
        Input::SendMessage("Hello Chat".to_string()),
        Input::SendMessage("I am new here".to_string()),
        Input::SendMessage("Bye".to_string()),
        // Input::Disconnect,
    ];

    for command in commands {
        tracing::info!("SENDING COMMAND: {:?}" ,command);
        let output = client.transition(command.clone()).await;
        tracing::info!("RESPONSE: {:?}", output);
    }
    tracing::info!("DONE");
    Ok(())
}

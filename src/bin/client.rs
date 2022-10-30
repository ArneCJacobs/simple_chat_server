use std::io::{stdin, self, Write};
use std::error::Error;

use tracing::Level;

use simple_chat_protocol::protocol::client::{ClientSideConnectionSM, Input, NotConnected, Shared, Reaction, InputParseError};
use rust_state_machine::{StateMachineAsync, StatefulAsyncStateMachine};

const ADDR: &str = "127.0.0.1:8080";

fn prompt(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    
    let mut buffer = String::new();
    stdin().read_line(&mut buffer).unwrap();
    return buffer.trim().to_string();
}


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

    // let args: Vec<String> = env::args().collect();
    // tokio::time::sleep(Duration::from_secs_f64(0.1)).await;
    let shared = Shared;
    let start_state = NotConnected;
    let mut client: StateMachineAsync<ClientSideConnectionSM> = StatefulAsyncStateMachine::init(shared, start_state);
    // let commands = vec![
    //     Input::ConnectServer(ADDR.to_string()),
    //     Input::Authenticate(args[1].clone()),
    //     Input::GetChannelsList,
    //     Input::ConnectChannel("Welcome".to_string()),
    //     Input::SendMessage("Hello Chat".to_string()),
    //     Input::SendMessage("I am new here".to_string()),
    //     Input::SendMessage("Bye".to_string()),
    //     Input::Disconnect,
    // ];
    //
    // for command in commands {
    //     if let Input::SendMessage(_) = command {
    //         tokio::time::sleep(Duration::from_secs_f64(4.0)).await;
    //     } 
    //     tracing::info!("SENDING COMMAND: {:?}" ,command);
    //     let output = client.transition(command.clone()).await;
    //     tracing::info!("RESPONSE: {:?}", output);
    // }
    loop {
        let addr = match prompt("Please provide the server address, or empty for local: ").as_str() {
            "" => ADDR.to_string(),
            val => val.to_string(), 
        };
        tracing::info!("Connecting to {}", addr);
        let res = client.transition(Input::ConnectServer(addr)).await;
        if let Some(Reaction::Success) = res {
            break;
        } else if let Some(Reaction::Deny { error }) = res {
            println!("Error: {:?}", error);
        } else {
            return Ok(());
        }
    }

    loop {
        let username = prompt("Please enter a username: ");
        tracing::info!("Authenticating with {}", username);
        let res = client.transition(Input::Authenticate(username)).await;
        if let Some(Reaction::Success) = res {
            break;
        } else if let Some(Reaction::Deny { error }) = res {
            println!("Error: {:?}", error);
        } else {
            return Ok(());
        }
    }

    loop {
        let command: Result<Input, InputParseError> = prompt("Command: ").parse();
        println!("COMMAND: {:?}", command);
        let command = match command {
            Ok(val) => val,
            Err(err) => {
                println!("Command parse error: {:?}", err);
                continue;
            },
        };

        let res = client.transition(command).await;
        println!("Res: {:?}", res);
        if res.is_none() {
            tracing::info!("DONE");
            return Ok(());
        }
    }
}

use tokio::net::{TcpListener, TcpStream};
use std::{env, error::Error};
use tokio::stream::StreamExt;

use std::collections::HashMap;
use std::sync::{Mutex};

mod ndi;

use ndi::{FindInstance};

use log::{error, info, warn};
use log4rs;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

struct VideoRouterState {
    map: Mutex<HashMap<String, String>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();

    info!("starting ndi-router {}", VERSION);

    if !ndi::initialize() {
        panic!("Cannot initialize NDI libs");
    }

    let mut find = match FindInstance::builder().build() {
        None => panic!(Some("Cannot initialize NDI libs")),
        Some(find) => find,
    };

    let new_sources = find.wait_for_sources(100);
    let sources = find.get_current_sources();

    if new_sources {
        for source in &sources {
            info!("Found source '{}' with IP {}",  source.ndi_name(), source.ip_address());
        }
    } else {
        error!("No NDI sources found");
        return Ok(())
    }

    // Parse the arguments, bind the TCP socket we'll be listening to, spin up
    // our worker threads, and start shipping sockets to those worker threads.
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9909".to_string());
    let mut server = TcpListener::bind(&addr).await?;
    let mut incoming = server.incoming();
    info!("Listening on: {}", addr);

    while let Some(Ok(stream)) = incoming.next().await {
        tokio::spawn(async move {
            if let Err(e) = process(stream).await {
                println!("failed to process connection; error = {}", e);
            }
        });
    }

    Ok(())
}

async fn process(stream: TcpStream) -> Result<(), Box<dyn Error>> {
    Ok(())
}
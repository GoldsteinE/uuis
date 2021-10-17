#![allow(dead_code)]

pub mod server;
pub mod types;
pub mod ui;
pub mod util;

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt().init();
    server::Server::run_tcp("127.0.0.1:5555")?;
    Ok(())
}

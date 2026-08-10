mod commands;
pub mod discord;
mod embedbuilder;
pub mod server;
mod test;

pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/discord_shim.rs"));
}

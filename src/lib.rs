mod embedbuilder;
pub mod server;
mod test;
pub mod discord;
mod commands;

pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/discord_shim.rs"));
}

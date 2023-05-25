use crate::server::discord_shim::{ProtoFile, Response};
use async_std::io::{ReadExt, WriteExt};
use async_std::net::TcpListener;
use async_std::net::TcpStream;
use async_std::sync::Mutex;
use byteorder::{ByteOrder, LittleEndian};
use futures::stream::StreamExt;
use prost::Message;
use serenity::client::Context;
use serenity::model::id::{ChannelId, UserId};
use serenity::model::prelude::Activity;
use serenity::model::prelude::OnlineStatus;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

pub mod discord_shim {
    include!(concat!(env!("OUT_DIR"), "/discord_shim.rs"));
}

struct DiscordSettings {
    channel: u64,
    // Only relevant when self-hosting, global discordshim wont support presence anyway
    prefix: String,
    cycle_time: i32,
    enabled: bool,
}

pub(crate) struct Server {
    clients: Arc<HashMap<ChannelId, Mutex<TcpStream>>>,
}

impl Server {
    pub(crate) fn new() -> Server {
        return Server {
            clients: Arc::new(HashMap::new()),
        };
    }

    pub(crate) async fn run(&self, ctx: Arc<Context>) {
        let listener = TcpListener::bind("0.0.0.0:12345")
            .await
            .expect("Failed to bind");
        listener
            .incoming()
            .for_each_concurrent(None, |tcpstream| {
                let ctx2 = ctx.clone();
                async move {
                    let tcpstream = tcpstream.unwrap();
                    let f = ctx2.clone();
                    self.connection_loop(tcpstream, f).await.unwrap();
                }
            })
            .await;
    }

    async fn connection_loop(&self, mut stream: TcpStream, ctx: Arc<Context>) -> Result<(), ()> {
        let mut settings = DiscordSettings {
            channel: 0,
            prefix: "".to_string(),
            cycle_time: 0,
            enabled: false,
        };
        loop {
            let length_buf = &mut [0u8; 4];
            stream.read_exact(length_buf).await.unwrap();
            let length = LittleEndian::read_u32(length_buf) as usize;

            let mut buf = vec![0u8; length];
            stream.read_exact(&mut buf).await.unwrap();

            let response = Response::decode(&mut Cursor::new(buf)).unwrap();

            self.handle_task(&mut settings, response, ctx.clone()).await;
        }
    }

    async fn handle_task(
        &self,
        settings: &mut DiscordSettings,
        response: Response,
        ctx: Arc<Context>,
    ) {
        let channel = ChannelId(settings.channel);
        if response.file.is_some() {
            channel
                .send_message(&ctx, |m| {
                    m.embed(|e| {
                        e.title("System Resource Load");
                        e.field("CPU Load Average", format!("{:.2}%", 10.0), false);
                        e.field(
                            "Memory Usage",
                            format!("{:.2} MB Free out of {:.2} MB", 1000.0, 1000.0),
                            false,
                        );
                        e
                    })
                })
                .await
                .unwrap();
        }
        if response.embed.is_some() {
            let response_embed = response.embed.unwrap();
            channel
                .send_message(&ctx, |m| {
                    m.embed(|e| {
                        e.title(response_embed.title);
                        e.description(response_embed.description);
                        e
                    })
                })
                .await
                .unwrap();
        }
        if response.presence.is_some() {
            let activity = Activity::playing(response.presence.unwrap().presence);
            ctx.shard.set_presence(Some(activity), OnlineStatus::Online);
        }
        if response.settings.is_some() {
            let new_settings = response.settings.unwrap();
            settings.channel = new_settings.channel_id;
            settings.prefix = new_settings.command_prefix;
            settings.cycle_time = new_settings.cycle_time;
            settings.enabled = new_settings.presence_enabled;
        }
    }

    pub(crate) async fn send_command(&self, channel: ChannelId, user: UserId, command: String) {
        let mut request = discord_shim::Request::default();
        request.user = user.0;
        request.command = command;
        let data = request.encode_to_vec();

        self._send_data(channel, data).await
    }

    async fn _send_data(&self, channel: ChannelId, data: Vec<u8>) {
        let length = data.len() as u32;
        let length_buf = &mut [0u8; 4];
        LittleEndian::write_u32(length_buf, length);

        for client in self.clients.keys() {
            if channel.0 == client.0 {
                let mut tcpstream = self.clients.get(client).unwrap().lock().await;

                tcpstream.write_all(length_buf).await.unwrap();
                tcpstream.write_all(&*data).await.unwrap();
            }
        }
    }

    pub(crate) async fn send_file(
        &self,
        channel: ChannelId,
        user: UserId,
        filename: String,
        file: Vec<u8>,
    ) {
        let mut request = discord_shim::Request::default();
        request.user = user.0;
        let mut req_file = ProtoFile::default();
        req_file.data = file;
        req_file.filename = filename;
        request.file = Option::Some(req_file);
        let data = request.encode_to_vec();

        self._send_data(channel, data).await
    }
}

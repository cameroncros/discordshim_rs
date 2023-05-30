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
use serenity::model::prelude::OnlineStatus;
use serenity::model::prelude::{Activity, AttachmentType};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::io::Cursor;
use std::sync::Arc;

pub mod discord_shim {
    include!(concat!(env!("OUT_DIR"), "/discord_shim.rs"));
}

struct DiscordSettings {
    channel: u64,
    // Only relevant when self-hosting, global discordshim won't support presence anyway
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
        let listener = TcpListener::bind("127.0.0.1:12345")
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

    async fn connection_loop(
        &self,
        mut stream: TcpStream,
        ctx: Arc<Context>,
    ) -> Result<String, io::Error> {
        let mut settings = DiscordSettings {
            channel: 0,
            prefix: "".to_string(),
            cycle_time: 0,
            enabled: false,
        };
        loop {
            let length_buf = &mut [0u8; 4];
            match stream.read_exact(length_buf).await {
                Ok(_) => {}
                Err(message) => return Ok(message.to_string()),
            }
            let length = LittleEndian::read_u32(length_buf) as usize;

            let mut buf = vec![0u8; length];
            match stream.read_exact(&mut buf).await {
                Ok(_) => {}
                Err(message) => return Ok(message.to_string()),
            }

            let result = Response::decode(&mut Cursor::new(buf));
            if result.is_err() {
                return Ok("Failed to decode message".to_string());
            }
            let response = result.unwrap();

            self.handle_task(&mut settings, response, ctx.clone()).await;
        }
    }

    async fn handle_task(
        &self,
        settings: &mut DiscordSettings,
        response: discord_shim::Response,
        ctx: Arc<Context>,
    ) {
        let channel = ChannelId(settings.channel);
        if response.file.is_some() {
            let protofile: ProtoFile = response.file.unwrap();

            let filename = protofile.filename.clone();
            let filedata = protofile.data.as_slice();
            let files = vec![AttachmentType::Bytes {
                data: Cow::from(filedata),
                filename: filename.clone(),
            }];

            channel
                .send_files(&ctx, files, |m| m.content(filename))
                .await
                .unwrap();
        }
        if response.embed.is_some() {
            let response_embed = response.embed.unwrap();
            let embeds = self._subdivide_embeds(response_embed);
            for e in embeds {
                if e.snapshot.is_some() {
                    let snapshot = e.snapshot.clone().unwrap();
                    let filename = format!("{}", snapshot.filename);
                    let filename2 = format!("attachment://{}", snapshot.filename);
                    let filedata = snapshot.data.as_slice();
                    let files = vec![AttachmentType::Bytes {
                        data: Cow::from(filedata),
                        filename: filename.clone(),
                    }];
                    channel
                        .send_files(&ctx, files, |m| {
                            m.embed(|f| {
                                f.title(e.title.clone())
                                    .description(e.description)
                                    .color(e.color)
                                    .author(|a| a.name(e.author));
                                for field in e.textfield {
                                    f.field(field.title, field.text, field.inline);
                                }
                                f.image(filename2.clone());
                                f
                            })
                        })
                        .await
                        .unwrap();
                } else {
                    channel
                        .send_message(&ctx, |m| {
                            m.embed(|f| {
                                f.title(e.title.clone())
                                    .description(e.description)
                                    .color(e.color)
                                    .author(|a| a.name(e.author));
                                for field in e.textfield {
                                    f.field(field.title, field.text, field.inline);
                                }
                                f
                            })
                        })
                        .await
                        .unwrap();
                }
            }
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
        request.file = Some(req_file);
        let data = request.encode_to_vec();

        self._send_data(channel, data).await
    }

    fn _subdivide_embeds(
        &self,
        embed_content: discord_shim::EmbedContent,
    ) -> Vec<discord_shim::EmbedContent> {
        let mut embeds = vec![];
        let mut first = discord_shim::EmbedContent::default();
        first.title = embed_content.title;
        first.description = embed_content.description;
        first.snapshot = embed_content.snapshot;

        first.author = embed_content.author.clone();
        first.color = embed_content.color;

        let mut last = first;

        for field in embed_content.textfield {
            if last.textfield.len() >= 25 {
                embeds.push(last);
                last = discord_shim::EmbedContent::default();
                last.author = embed_content.author.clone();
                last.color = embed_content.color;
            }

            last.textfield.push(field.clone());
        }

        embeds.push(last);

        return embeds;
    }
}

use crate::server::discord_shim::{ProtoFile, Response};
use async_std::io::{ReadExt, WriteExt};
use async_std::net::TcpListener;
use async_std::net::TcpStream;
use async_std::sync::{Mutex, RwLock};
use byteorder::{ByteOrder, LittleEndian};
use futures::stream::StreamExt;
use prost::Message;

use serenity::client::Context;
use serenity::model::id::{ChannelId, UserId};
use serenity::model::prelude::OnlineStatus;
use serenity::model::prelude::{Activity, AttachmentType};
use std::borrow::Cow;
use std::io;
use std::io::Cursor;
use std::sync::Arc;

pub mod discord_shim {
    include!(concat!(env!("OUT_DIR"), "/discord_shim.rs"));
}

struct DiscordSettings {
    tcpstream: RwLock<TcpStream>,
    channel: RwLock<ChannelId>,
    // Only relevant when self-hosting, global discordshim won't support presence anyway
    prefix: Mutex<String>,
    cycle_time: Mutex<i32>,
    enabled: Mutex<bool>,
}

pub(crate) struct Server {
    clients: Arc<Mutex<Vec<Arc<DiscordSettings>>>>,
}

impl Server {
    pub(crate) fn new() -> Server {
        return Server {
            clients: Arc::new(Mutex::new(Vec::new())),
        };
    }

    pub(crate) async fn run(&self, ctx: Arc<Context>) {
        println!("Starting TCP listener");
        let listener = TcpListener::bind("0.0.0.0:23416")
            .await
            .expect("Failed to bind");
        listener
            .incoming()
            .for_each_concurrent(None, |tcpstream| {
                let ctx2 = ctx.clone();
                let clients2 = self.clients.clone();
                async move {
                    let f = ctx2.clone();
                    let c = clients2.clone();
                    let stream = tcpstream.unwrap();
                    println!("Received connection from: {}", stream.peer_addr().unwrap());

                    let settings = Arc::new(DiscordSettings {
                        tcpstream: RwLock::new(stream.clone()),
                        channel: RwLock::new(ChannelId(0)),
                        prefix: Mutex::new("".to_string()),
                        cycle_time: Mutex::new(0),
                        enabled: Mutex::new(false),
                    });
                    let settings2 = settings.clone();
                    let settings3 = settings.clone();

                    c.lock().await.insert(0, settings);

                    let loop_res = self.connection_loop(stream, settings2, f).await;
                    c.lock()
                        .await
                        .retain(|item| !Arc::<DiscordSettings>::ptr_eq(&item, &settings3));

                    loop_res.expect("Loop failed");
                }
            })
            .await;
    }

    async fn connection_loop(
        &self,
        mut stream: TcpStream,
        settings: Arc<DiscordSettings>,
        ctx: Arc<Context>,
    ) -> Result<String, io::Error> {
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

            self.handle_task(settings.clone(), response, ctx.clone())
                .await;
        }
    }

    async fn handle_task(
        &self,
        settings: Arc<DiscordSettings>,
        response: discord_shim::Response,
        ctx: Arc<Context>,
    ) {
        match response.field {
            None => {}
            Some(discord_shim::response::Field::File(protofile)) => {
                let filename = protofile.filename.clone();
                let filedata = protofile.data.as_slice();
                let files = vec![AttachmentType::Bytes {
                    data: Cow::from(filedata),
                    filename: filename.clone(),
                }];

                settings
                    .channel
                    .read()
                    .await
                    .send_files(&ctx, files, |m| m.content(filename))
                    .await
                    .unwrap();
            }

            Some(discord_shim::response::Field::Embed(response_embed)) => {
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
                        settings
                            .channel
                            .read()
                            .await
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
                        settings
                            .channel
                            .read()
                            .await
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

            Some(discord_shim::response::Field::Presence(presence)) => {
                let activity = Activity::playing(presence.presence);
                ctx.shard.set_presence(Some(activity), OnlineStatus::Online);
            }

            Some(discord_shim::response::Field::Settings(new_settings)) => {
                *settings.channel.write().await = ChannelId(new_settings.channel_id);
                *settings.prefix.lock().await = new_settings.command_prefix;
                *settings.cycle_time.lock().await = new_settings.cycle_time;
                *settings.enabled.lock().await = new_settings.presence_enabled;
            }
        }
    }

    pub(crate) async fn send_command(&self, channel: ChannelId, user: UserId, command: String) {
        let mut request = discord_shim::Request::default();
        request.user = user.0;
        request.message = Some(discord_shim::request::Message::Command(command));
        let data = request.encode_to_vec();

        self._send_data(channel, data).await
    }

    async fn _send_data(&self, channel: ChannelId, data: Vec<u8>) {
        let length = data.len() as u32;
        let length_buf = &mut [0u8; 4];
        LittleEndian::write_u32(length_buf, length);

        let c = self.clients.lock().await;

        for client in c.as_slice() {
            if channel.0 != 0 && channel.0 == client.channel.read().await.0 {
                let mut tcpstream = client.tcpstream.write().await;

                if tcpstream.write_all(length_buf).await.is_err() {
                    continue;
                }
                if tcpstream.write_all(&*data).await.is_err() {
                    continue;
                }
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
        request.message = Some(discord_shim::request::Message::File(req_file));
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

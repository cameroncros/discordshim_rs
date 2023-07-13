use crate::embedbuilder::{build_embeds, split_file};
use crate::messages;
use async_std::io::{ReadExt, WriteExt};
use async_std::net::TcpListener;
use async_std::net::TcpStream;
use async_std::sync::{Mutex, RwLock};
use byteorder::{ByteOrder, LittleEndian};
use futures::stream::StreamExt;
use log::{debug, error, info};
use protobuf::Message;
use serenity::client::Context;
use serenity::model::id::{ChannelId, UserId};
use serenity::model::prelude::OnlineStatus;
use serenity::model::prelude::{Activity, AttachmentType};
use std::borrow::Cow;
use std::env;
use std::sync::Arc;

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
        debug!("Starting TCP listener");
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
                    let peer_addr = stream.peer_addr().unwrap().clone();
                    info!("Received connection from: {}", peer_addr);

                    let settings = Arc::new(DiscordSettings {
                        tcpstream: RwLock::new(stream.clone()),
                        channel: RwLock::new(ChannelId(0)),
                        prefix: Mutex::new("".to_string()),
                        cycle_time: Mutex::new(0),
                        enabled: Mutex::new(false),
                    });

                    c.lock().await.insert(0, settings.clone());

                    let cloud = env::var("CLOUD_SERVER");
                    if cloud.is_ok() {
                        let presence = format!("to {} instances", c.lock().await.len());
                        ctx2.clone()
                            .set_presence(
                                Option::Some(Activity::streaming(
                                    presence,
                                    "https://octoprint.org",
                                )),
                                OnlineStatus::Online,
                            )
                            .await;
                    }

                    let _loop_res = self.connection_loop(stream, settings.clone(), f).await;
                    c.lock()
                        .await
                        .retain(|item| !Arc::<DiscordSettings>::ptr_eq(&item, &settings));

                    if cloud.is_ok() {
                        let presence = format!("to {} instances", c.lock().await.len());
                        ctx2.clone()
                            .set_presence(
                                Option::Some(Activity::streaming(
                                    presence,
                                    "https://octoprint.org",
                                )),
                                OnlineStatus::Online,
                            )
                            .await;
                    }

                    info!("Dropped connection from: {}", peer_addr);
                }
            })
            .await;
    }

    async fn connection_loop(
        &self,
        mut stream: TcpStream,
        settings: Arc<DiscordSettings>,
        ctx: Arc<Context>,
    ) {
        loop {
            let length_buf = &mut [0u8; 4];
            match stream.read_exact(length_buf).await {
                Ok(_) => {}
                Err(message) => {
                    debug!("Read length failed with [{message}]");
                    return;
                }
            }
            let length = LittleEndian::read_u32(length_buf) as usize;
            debug!("Incoming response, {length} bytes long.");

            let mut buf = vec![0u8; length];
            match stream.read_exact(&mut buf).await {
                Ok(_) => {}
                Err(message) => {
                    debug!("Read data failed with [{message}]");
                    return;
                }
            }

            let result = messages::Response::parse_from_bytes(buf.as_slice());
            if result.is_err() {
                debug!(
                    "Parse data failed with [{}]",
                    result.err().unwrap().to_string()
                );
                return;
            }
            let response = result.unwrap();

            let result = self
                .handle_task(settings.clone(), response, ctx.clone())
                .await;
            if result.is_err() {
                debug!("Failed to send response");
                return;
            }
        }
    }

    async fn handle_task(
        &self,
        settings: Arc<DiscordSettings>,
        response: messages::Response,
        ctx: Arc<Context>,
    ) -> Result<(), ()> {
        match response.field {
            None => {
                return Ok(());
            }
            Some(messages::response::Field::File(protofile)) => {
                let filename = protofile.filename.clone();
                let filedata = protofile.data.as_slice();
                let files = split_file(filename, filedata);
                for file in files {
                    let result = settings
                        .channel
                        .read()
                        .await
                        .send_files(&ctx, vec![file.1], |m| m.content(file.0))
                        .await;
                    if result.is_err() {
                        let error = result.err().unwrap();
                        error!("{error}");
                        return Err(());
                    }
                }
                return Ok(());
            }

            Some(messages::response::Field::Embed(response_embed)) => {
                let embeds = build_embeds(response_embed);
                for e in embeds {
                    if e.snapshot.is_some() {
                        let snapshot = e.snapshot.clone().unwrap();
                        let filename_url = format!("attachment://{}", snapshot.filename);
                        let filedata = snapshot.data.as_slice();
                        let files = vec![AttachmentType::Bytes {
                            data: Cow::from(filedata),
                            filename: snapshot.filename,
                        }];
                        let result = settings
                            .channel
                            .read()
                            .await
                            .send_files(&ctx, files, |m| {
                                m.embed(|f| {
                                    f.title(e.title)
                                        .description(e.description)
                                        .color(e.color)
                                        .author(|a| a.name(e.author));
                                    for field in e.textfield {
                                        f.field(field.title, field.text, field.inline);
                                    }
                                    f.image(filename_url.clone());
                                    f
                                })
                            })
                            .await;
                        if result.is_err() {
                            let error = result.err().unwrap();
                            error!("{error}");
                            return Err(());
                        }
                    } else {
                        let result = settings
                            .channel
                            .read()
                            .await
                            .send_message(&ctx, |m| {
                                m.embed(|f| {
                                    f.title(e.title)
                                        .description(e.description)
                                        .color(e.color)
                                        .author(|a| a.name(e.author));
                                    for field in e.textfield {
                                        f.field(field.title, field.text, field.inline);
                                    }
                                    f
                                })
                            })
                            .await;
                        if result.is_err() {
                            let error = result.err().unwrap();
                            error!("{error}");
                            return Err(());
                        }
                    }
                }
                return Ok(());
            }

            Some(messages::response::Field::Presence(presence)) => {
                let cloud = env::var("CLOUD_SERVER");
                if cloud.is_err() {
                    let activity = Activity::playing(presence.presence);
                    ctx.shard.set_presence(Some(activity), OnlineStatus::Online);
                }
                return Ok(());
            }

            Some(messages::response::Field::Settings(new_settings)) => {
                *settings.channel.write().await = ChannelId(new_settings.channel_id);
                *settings.prefix.lock().await = new_settings.command_prefix;
                *settings.cycle_time.lock().await = new_settings.cycle_time;
                *settings.enabled.lock().await = new_settings.presence_enabled;
                return Ok(());
            }
        }
    }

    pub(crate) async fn send_command(&self, channel: ChannelId, user: UserId, command: String) {
        let mut request = messages::Request::default();
        request.user = user.0;
        request.message = Some(messages::request::Message::Command(command));
        let data = request.write_to_bytes().unwrap();

        self._send_data(channel, data).await
    }

    async fn _send_data(&self, channel: ChannelId, data: Vec<u8>) {
        let length = data.len() as u32;
        let length_buf = &mut [0u8; 4];
        LittleEndian::write_u32(length_buf, length);

        let c = self.clients.lock().await;

        let mut found = 0;
        for client in c.as_slice() {
            if channel.0 != 0 && channel.0 == client.channel.read().await.0 {
                let mut tcpstream = client.tcpstream.write().await;

                if tcpstream.write_all(length_buf).await.is_err() {
                    error!("Failed to send length");
                    continue;
                }
                if tcpstream.write_all(&*data).await.is_err() {
                    error!("Failed to send message");
                    continue;
                }
                found += 1;
            }
        }
        info!("Sent message to {found} clients");
    }

    pub(crate) async fn send_file(
        &self,
        channel: ChannelId,
        user: UserId,
        filename: String,
        file: Vec<u8>,
    ) {
        let mut request = messages::Request::default();
        request.user = user.0;
        let mut req_file = messages::ProtoFile::default();
        req_file.data = file;
        req_file.filename = filename;
        request.message = Some(messages::request::Message::File(req_file));
        let data = request.write_to_bytes().unwrap();

        self._send_data(channel, data).await
    }
}

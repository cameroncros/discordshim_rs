use crate::messages::response::Field;
use crate::messages::{EmbedContent, ProtoFile, Request, Response};
use crate::embedbuilder::{build_embeds, split_file};
use async_std::io::{ReadExt, WriteExt};
use async_std::net::TcpListener;
use async_std::net::TcpStream;
use async_std::sync::{Mutex, RwLock};
use byteorder::{ByteOrder, LittleEndian};
use csv::Writer;
use futures::stream::StreamExt;
use log::{debug, error, info};
use regex::Regex;
use serenity::client::Context;
use serenity::model::id::{ChannelId, UserId};
use serenity::model::prelude::OnlineStatus;
use std::borrow::Cow;
use std::env;
use std::sync::Arc;
use std::time::SystemTime;
use color_eyre::eyre;
use prost::Message;
use serenity::all::{ActivityData, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage};
use crate::messages::request::Message::{Command, File};

#[derive(serde::Serialize)]
struct Stats {
    ip: String,
    num_messages: u64,
    total_data: usize,
}

struct DiscordSettings {
    tcpstream: RwLock<TcpStream>,
    channel: RwLock<ChannelId>,
    // Only relevant when self-hosting, global discordshim won't support presence anyway
    prefix: Mutex<String>,
    cycle_time: Mutex<i32>,
    enabled: Mutex<bool>,
    num_messages: Mutex<u64>,
    total_data: Mutex<usize>,
}

impl DiscordSettings {
    async fn get_stats(&self) -> Stats {
        Stats {
            ip: self
                .tcpstream
                .read()
                .await
                .peer_addr()
                .unwrap()
                .to_string()
                .clone(),
            num_messages: *self.num_messages.lock().await,
            total_data: *self.total_data.lock().await,
        }
    }
}

pub struct Server {
    clients: Arc<Mutex<Vec<Arc<DiscordSettings>>>>,
    last_presense_update: Mutex<SystemTime>,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    pub fn new() -> Server {
        Server {
            clients: Arc::new(Mutex::new(Vec::new())),
            last_presense_update: Mutex::new(SystemTime::UNIX_EPOCH),
        }
    }

    pub async fn run(&self, ctx: Arc<Context>) {
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
                    let stream = match tcpstream {
                        Err(e) => {
                            error!("TCP stream error {:?}", e);
                            return;
                        }
                        Ok(s) => {
                            s
                        }
                    };

                    let peer_addr = stream.peer_addr().unwrap();
                    info!("Received connection from: {}", peer_addr);

                    let settings = Arc::new(DiscordSettings {
                        tcpstream: RwLock::new(stream.clone()),
                        channel: RwLock::new(ChannelId::default()),
                        prefix: Mutex::new(String::new()),
                        cycle_time: Mutex::new(0),
                        enabled: Mutex::new(false),
                        num_messages: Mutex::new(0),
                        total_data: Mutex::new(0),
                    });

                    clients2.lock().await.insert(0, settings.clone());

                    let num_servers = clients2.lock().await.len();
                    self.update_presence(ctx2.clone(), num_servers).await;

                    let _loop_res = self.connection_loop(stream, settings.clone(), ctx2.clone()).await;
                    clients2.lock()
                        .await
                        .retain(|item| !Arc::<DiscordSettings>::ptr_eq(item, &settings));

                    let num_servers = clients2.lock().await.len();
                    self.update_presence(ctx2, num_servers).await;

                    info!("Dropped connection from: {}", peer_addr);
                }
            })
            .await;
    }

    async fn update_presence(&self, ctx: Arc<Context>, num_servers: usize) {
        let mut last_update = self.last_presense_update.lock().await;
        let now = SystemTime::now();
        if now.duration_since(*last_update).unwrap().as_secs() < 60 {
            return;
        }

        let cloud = env::var("CLOUD_SERVER");
        if cloud.is_ok() {
            let presence = format!("to {num_servers} instances");
            ctx.set_presence(
                Some(ActivityData::streaming(presence, "https://octoprint.org").unwrap()),
                OnlineStatus::Online,
            );
        }

        *last_update = now;
    }

    async fn connection_loop(
        &self,
        mut stream: TcpStream,
        settings: Arc<DiscordSettings>,
        ctx: Arc<Context>,
    ) -> eyre::Result<()> {
        loop {
            let length_buf = &mut [0u8; 4];
            stream.read_exact(length_buf).await?;
            let length = LittleEndian::read_u32(length_buf) as usize;
            debug!("Incoming response, {length} bytes long.");

            let mut buf = vec![0u8; length];
            stream.read_exact(&mut buf).await?;

            let response = Response::decode(buf.as_slice())?;

            self.handle_task(settings.clone(), response, ctx.clone()).await?;
        }
    }

    async fn handle_task(
        &self,
        settings: Arc<DiscordSettings>,
        response: Response,
        ctx: Arc<Context>,
    ) -> eyre::Result<()> {
        *settings.num_messages.lock().await += 1;
        *settings.total_data.lock().await += response.encoded_len();
        match response.field {
            None => Ok(()),
            Some(Field::File(protofile)) => {
                let filename = protofile.filename.clone();
                let filedata = protofile.data.as_slice();
                let files = split_file(filename, filedata);
                for file in files {
                    let filename = file.1.filename.clone();
                    let file_builder = CreateMessage::new().add_file(CreateAttachment::bytes(file.0, filename));
                    settings
                        .channel
                        .read()
                        .await
                        .send_files(&ctx, vec![file.1], file_builder)
                        .await?;
                }
                Ok(())
            }

            Some(Field::Embed(response_embed)) => {
                let embeds = build_embeds(response_embed);
                for e in embeds {
                    let mentions = extract_mentions(&e);

                    if e.snapshot.is_some() {
                        let snapshot = e.snapshot.clone().unwrap();
                        let filename_url = format!("attachment://{}", snapshot.filename);
                        let filedata = snapshot.data.as_slice();
                        let files = vec![CreateAttachment::bytes (
                            Cow::from(filedata),
                            snapshot.filename,
                        )];
                        let mut embed = CreateEmbed::new().title(e.title)
                            .description(e.description)
                            .color(e.color)
                            .author(CreateEmbedAuthor::new(e.author))
                            .image(filename_url.clone());
                        for field in e.textfield {
                            embed = embed.field(field.title, field.text, field.inline);
                        }
                        let message = CreateMessage::new().embed(embed).content(mentions);
                        settings
                            .channel
                            .read()
                            .await
                            .send_files(&ctx, files, message)
                            .await?;
                    } else {
                        let mut embed = CreateEmbed::new().title(e.title)
                            .description(e.description)
                            .color(e.color)
                            .author(CreateEmbedAuthor::new(e.author));
                        for field in e.textfield {
                            embed = embed.field(field.title, field.text, field.inline);
                        }
                        let message = CreateMessage::new().embed(embed).content(mentions);
                        
                        settings
                            .channel
                            .read()
                            .await
                            .send_message(&ctx, message)
                            .await?;
                    }
                }
                Ok(())
            }

            Some(Field::Presence(presence)) => {
                let cloud = env::var("CLOUD_SERVER");
                if cloud.is_err() {
                    let activity = ActivityData::playing(presence.presence);
                    ctx.shard.set_presence(Some(activity), OnlineStatus::Online);
                }
                Ok(())
            }

            Some(Field::Settings(new_settings)) => {
                *settings.channel.write().await = ChannelId::from(new_settings.channel_id);
                *settings.prefix.lock().await = new_settings.command_prefix;
                *settings.cycle_time.lock().await = new_settings.cycle_time;
                *settings.enabled.lock().await = new_settings.presence_enabled;
                Ok(())
            }
        }
    }

    pub async fn send_command(&self, channel: ChannelId, user: UserId, command: String) -> eyre::Result<()>{
        let request = Request {
            user: user.get(),
            message: Some(Command(command)),
        };
        let data = request.encode_to_vec();

        self._send_data(channel, data).await
    }

    async fn _send_data(&self, channel: ChannelId, data: Vec<u8>) -> eyre::Result<()> {
        let length = u32::try_from(data.len())?;
        let length_buf = &mut [0u8; 4];
        LittleEndian::write_u32(length_buf, length);

        let c = self.clients.lock().await;

        let mut found = 0;
        for client in c.as_slice() {
            if channel.get() != 0 && channel.get() == client.channel.read().await.get() {
                let mut tcpstream = client.tcpstream.write().await;

                if tcpstream.write_all(length_buf).await.is_err() {
                    error!("Failed to send length");
                    continue;
                }
                if tcpstream.write_all(&data).await.is_err() {
                    error!("Failed to send message");
                    continue;
                }
                found += 1;
            }
        }
        info!("Sent message to {found} clients");
        Ok(())
    }

    pub async fn send_file(
        &self,
        channel: ChannelId,
        user: UserId,
        filename: String,
        file: Vec<u8>,
    ) -> eyre::Result<()> {
        let req_file = ProtoFile {
            data: file,
            filename,
        };

        let request = Request {
            user: user.get(),
            message: Some(File(req_file)),
        };

        let data = request.encode_to_vec();

        self._send_data(channel, data).await
    }

    pub async fn send_stats(&self, channel: ChannelId, ctx: Context) {
        let mut wtr = Writer::from_writer(vec![]);
        let c = self.clients.lock().await;
        for client in c.as_slice() {
            wtr.serialize(client.get_stats().await).unwrap();
        }
        wtr.flush().unwrap();

        let files = vec![CreateAttachment::bytes(
            Cow::from(wtr.into_inner().unwrap()),
            String::from("stats.csv"),
        )];
        let result = channel.send_files(&ctx, files, CreateMessage::new()).await;
        if result.is_err() {
            let error = result.err().unwrap();
            error!("{error}");
        }
    }
}

fn extract_mentions(e: &EmbedContent) -> String {
    let mut mentions = String::new();
    let re = Regex::new(r"(<@[0-9a-zA-Z]*>)").unwrap();
    for (_, [mention]) in re.captures_iter(e.title.as_str()).map(|c| c.extract()) {
        mentions = mentions + mention + " ";
    }
    for (_, [mention]) in re
        .captures_iter(e.description.as_str())
        .map(|c| c.extract())
    {
        mentions = mentions + mention + " ";
    }
    mentions
}

#[cfg(test)]
mod tests {
    use crate::messages::EmbedContent;
    use crate::server::extract_mentions;

    #[test]
    fn test_extract_mentions_empty() {
        let e = EmbedContent::default();
        let mentions = extract_mentions(&e);
        assert_eq!("", mentions);
    }

    #[test]
    fn test_extract_mentions_title() {
        let mut e = EmbedContent::default();
        e.title = "<@12345678910> <@Everyone>".to_string();
        let mentions = extract_mentions(&e);
        assert_eq!("<@12345678910> <@Everyone> ", mentions);
    }

    #[test]
    fn test_extract_mentions_description() {
        let mut e = EmbedContent::default();
        e.description = "<@12345678910> <@Everyone>".to_string();
        let mentions = extract_mentions(&e);
        assert_eq!("<@12345678910> <@Everyone> ", mentions);
    }
}

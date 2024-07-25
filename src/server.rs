use crate::embedbuilder::{build_embeds, split_file};
use crate::messages;
use crate::messages::{EmbedContent, Request};
use csv::Writer;
use log::{debug, error, info};
use protobuf::Message;
use regex::Regex;
use serenity::client::Context;
use serenity::model::id::{ChannelId, UserId};
use serenity::model::prelude::OnlineStatus;
use std::borrow::Cow;
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64};
use std::sync::atomic::Ordering::Relaxed;
use std::time::SystemTime;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use color_eyre::eyre;
use color_eyre::eyre::eyre;
use serenity::all::{ActivityData, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::select;
use crate::helper::{receive_msg, send_request};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock};

#[derive(serde::Serialize)]
struct Stats {
    ip: String,
    num_messages: AtomicU64,
    total_data: AtomicU64,
}

struct Config {
    channelid: RwLock<ChannelId>,
    prefix: RwLock<String>,
    cycle_time: RwLock<i32>,
    enabled: RwLock<bool>,
}

pub(crate) struct DiscordSettings {
    sender: UnboundedSender<Request>,
    // Only relevant when self-hosting, global discordshim won't support presence anyway
    config: Config,
    stats: Stats
}

impl DiscordSettings {
    async fn get_stats(&self) -> &Stats {
        &self.stats
    }
}

type ClientList = Arc<RwLock<Vec<Arc<DiscordSettings>>>>;
type SharedSystemTime = Arc<RwLock<SystemTime>>;

pub(crate) struct Server {
    pub clients: ClientList,
    last_presense_update: SharedSystemTime,
}



async fn update_presence(last_presense_update: SharedSystemTime, ctx: Arc<Context>, num_servers: usize) {
    let now = SystemTime::now();
    
    {
        let last_update = last_presense_update.read().await;
        if now.duration_since(*last_update).unwrap().as_secs() < 60 {
            return;
        }
    }

    let cloud = env::var("CLOUD_SERVER");
    if cloud.is_ok() {
        let presence = format!("to {num_servers} instances");
        ctx.set_presence(
            Some(ActivityData::streaming(presence, "https://octoprint.org").unwrap()),
            OnlineStatus::Online,
        );
    }

    *last_presense_update.write().await = now;
}

async fn sender(stx: &mut (impl AsyncWriteExt + Unpin), mut rx: UnboundedReceiver<Request>) -> eyre::Result<()> {
    loop {
        let message = rx.recv().await;
        match message {
            None => {break}
            Some(m) => {
                send_request(stx, m).await?;
            }
        }
    }
    Ok(())
}

async fn receiver(rtx: &mut (impl AsyncReadExt + Unpin), settings: Arc<DiscordSettings>, ctx: Arc<Context>) -> eyre::Result<()> {
    loop {
        let response = receive_msg(rtx).await?;

        handle_task(settings.clone(), response, ctx.clone()).await?;
    }
}


async fn connection_loop(
    mut stream: TcpStream,
    rx: UnboundedReceiver<Request>,
    settings: Arc<DiscordSettings>,
    ctx: Arc<Context>,
) -> eyre::Result<()> {
    let (mut stx, mut srx) = stream.split();

    select!(
            _ = receiver(&mut stx, settings, ctx.clone()) => {}
            _ = sender(&mut srx, rx) => {}
        );
    Ok(())
}

async fn handle_task(
    settings: Arc<DiscordSettings>,
    response: messages::Response,
    ctx: Arc<Context>,
) -> eyre::Result<()> {
    settings.stats.num_messages.fetch_add(1, Relaxed);
    settings.stats.total_data.fetch_add(response.compute_size(), Relaxed);
    match response.field {
        None => Ok(()),
        Some(messages::response::Field::File(protofile)) => {
            let filename = protofile.filename.clone();
            let filedata = protofile.data.as_slice();
            let files = split_file(filename, filedata);
            for file in files {
                let filename = file.1.filename.clone();
                let file_builder = CreateMessage::new().add_file(CreateAttachment::bytes(file.0, filename));
                settings
                    .config.channelid
                    .read()
                    .await
                    .send_files(&ctx, vec![file.1], file_builder)
                    .await?;
            }
            Ok(())
        }

        Some(messages::response::Field::Embed(response_embed)) => {
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
                        .config.channelid
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
                        .config.channelid
                        .read()
                        .await
                        .send_message(&ctx, message)
                        .await?;
                }
            }
            Ok(())
        }

        Some(messages::response::Field::Presence(presence)) => {
            let cloud = env::var("CLOUD_SERVER");
            if cloud.is_err() {
                let activity = ActivityData::playing(presence.presence);
                ctx.shard.set_presence(Some(activity), OnlineStatus::Online);
            }
            Ok(())
        }

        Some(messages::response::Field::Settings(new_settings)) => {
            *settings.config.channelid.write().await = ChannelId::from(new_settings.channel_id);
            *settings.config.prefix.write().await = new_settings.command_prefix;
            *settings.config.cycle_time.write().await = new_settings.cycle_time;
            *settings.config.enabled.write().await = new_settings.presence_enabled;
            Ok(())
        }
    }
}

pub(crate) async fn send_command(channel: ChannelId, user: UserId, command: String, clients: ClientList) -> eyre::Result<()>{
    let mut request = messages::Request::default();
    request.user = user.get();
    request.message = Some(messages::request::Message::Command(command));

    _send_data(channel, request, clients).await
}

async fn _send_data(channel: ChannelId, request: Request, clients: ClientList) -> eyre::Result<()> {
    let c = clients.read().await;

    let mut found = 0;
    for client in c.iter() {
        if channel.get() != 0 && channel.get() == client.config.channelid.read().await.get() {
            client.sender.send(request.clone())?;
            found += 1;
        }
    }
    info!("Sent message to {found} clients");
    Ok(())
}

pub(crate) async fn send_file(
    channel: ChannelId,
    user: UserId,
    filename: String,
    file: Vec<u8>,
    clients: ClientList
) -> eyre::Result<()> {
    let req_file = messages::ProtoFile {
        data: file,
        filename,
        ..Default::default()
    };

    let request = Request {
        user: user.get(),
        message: Some(messages::request::Message::File(req_file)),
        ..Default::default()
    };

    _send_data(channel, request, clients).await
}

pub(crate) async fn send_stats(channel: ChannelId, ctx: Context, clients: ClientList) {
    let mut wtr = Writer::from_writer(vec![]);
    let c = clients.read().await;
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

async fn connection_handler(tcpstream: TcpStream,
                            ctx: Arc<Context>,
                            clients: ClientList,
                            last_update_time: SharedSystemTime
                            ) -> eyre::Result<()>
{
        let (tx, rx) = unbounded_channel();

        let peer_addr = match tcpstream.peer_addr() {
            Ok(p) => {p}
            Err(_) => {return Err(eyre!("Failed to get peeraddr from stream"))}
        };
        info!("Received connection from: {}", peer_addr);


        let settings = Arc::new(DiscordSettings {
            sender: tx,
            config: Config {
                channelid: RwLock::new(ChannelId::default()),
                prefix: RwLock::new(String::new()),
                cycle_time: RwLock::new(0),
                enabled: RwLock::new(false),
            },
            stats: Stats {
                ip: peer_addr.to_string(),
                num_messages: AtomicU64::new(0),
                total_data: AtomicU64::new(0),
            }
        });

        clients.write().await.insert(0, settings.clone());

        let num_servers = clients.read().await.len();
        update_presence(last_update_time.clone(), ctx.clone(), num_servers).await;

        connection_loop(tcpstream, rx, settings.clone(), ctx.clone()).await?;

        clients.write()
            .await
            .retain(|item| !Arc::<DiscordSettings>::ptr_eq(item, &settings));

        let num_servers = clients.read().await.len();
        update_presence(last_update_time.clone(), ctx.clone(), num_servers).await;

        info!("Dropped connection from: {}", peer_addr);

        Ok(())
}

impl Server {
    pub(crate) fn new() -> Server {
        Server {
            clients: Arc::new(RwLock::new(Vec::new())),
            last_presense_update: Arc::new(RwLock::new(SystemTime::UNIX_EPOCH)),
        }
    }

    pub(crate) async fn run(&self, ctx: Arc<Context>) {
        debug!("Starting TCP listener");
        let listener = TcpListener::bind("0.0.0.0:23416")
            .await
            .expect("Failed to bind");

        loop {
            match listener.accept().await {
                Ok((tcpstream, _)) => {
                    tokio::spawn(connection_handler(tcpstream, ctx.clone(), self.clients.clone(), self.last_presense_update.clone()));
                }
                Err(_) => {
                    tracing::error!("Failed to accept")
                }
            }
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
        let e = EmbedContent::new();
        let mentions = extract_mentions(&e);
        assert_eq!("", mentions);
    }

    #[test]
    fn test_extract_mentions_title() {
        let mut e = EmbedContent::new();
        e.title = "<@12345678910> <@Everyone>".to_string();
        let mentions = extract_mentions(&e);
        assert_eq!("<@12345678910> <@Everyone> ", mentions);
    }

    #[test]
    fn test_extract_mentions_description() {
        let mut e = EmbedContent::new();
        e.description = "<@12345678910> <@Everyone>".to_string();
        let mentions = extract_mentions(&e);
        assert_eq!("<@12345678910> <@Everyone> ", mentions);
    }
}

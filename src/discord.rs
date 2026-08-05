use std::env;
use std::sync::Arc;
use async_std::sync::RwLock;
use color_eyre::eyre;
use crate::commands::*;
use poise::{
    serenity_prelude as serenity};
use poise::serenity_prelude::{ChannelId, FullEvent};
use crate::server::Server;

pub struct Data {
    pub healthcheckchannel: ChannelId,
    pub server: Arc<RwLock<Server>>,
}
pub(crate) type Context<'a> = poise::Context<'a, Data, eyre::Error>;

async fn run_server(ctx: Arc<poise::serenity_prelude::Context>, server: Arc<RwLock<Server>>) {
    server.read().await.run(ctx).await;
}

pub async fn serve() -> eyre::Result<()> {
    let server = Arc::new(RwLock::new(Server::new()));
    let ctx_server = server.clone();

    let intents = serenity::GatewayIntents::non_privileged();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                connect(),
                disconnect(),
                print(),
files(),
unzip(),
abort(),
snapshot(),
status(),
help(),
pause(),
resume(),
timelapse(),
mute(),
unmute(),
gcode(),
getfile(),
gettimelapse(),
poweron(),
poweroff(),
powerstatus(),
listsystemcommands(),
systemcommand(),
            ],
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    event_handler(ctx, event, data).await
                })
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx.clone(), &framework.options().commands)
                    .await?;

                let channelid: u64 = env::var("HEALTH_CHECK_CHANNEL_ID")
                    .expect("channel id")
                    .parse()?;

                Ok(Data {
                    healthcheckchannel: ChannelId::from(channelid),
                    server: ctx_server,
                })
            })
        })
        .build();
    let token = env::var("DISCORD_TOKEN").expect("token");
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}

pub(crate) async fn event_handler(
    ctx: &serenity::Context,
    event: &FullEvent,
    data: &Data,
) -> eyre::Result<()> {
    if let FullEvent::Ready { data_about_bot: _data_about_bot } = event {
        let context = Arc::new(ctx.clone());
        tokio::spawn(run_server(context, data.server.clone()));
    }
    else if let FullEvent::Message { new_message } = event
    {
        if new_message.channel_id == data.healthcheckchannel && new_message.content == "/stats" {
            data.server
                .read()
                .await
                .send_stats(new_message.channel_id, ctx.clone())
                .await;
        }

        // Check for health check message.
        if new_message.author == **ctx.cache.current_user() {
            // Message is from ourselves.
            if new_message.channel_id == data.healthcheckchannel {
                if new_message.embeds.len() != 1 {
                    return Ok(());
                }
                let embed1 = new_message.embeds.first().unwrap();
                if embed1.title.is_none() {
                    return Ok(());
                }
                let flag = embed1.title.as_ref().unwrap().clone();
                let _ = data.server
                    .read()
                    .await
                    .send_command(new_message.channel_id, new_message.author.id, flag)
                    .await;
                return Ok(());
            }
            return Ok(());
        }

        if new_message.guild_id.is_none() {
            // is_private()
            return Ok(());
        }
        // Process all other messages as normal.
        let _ = data.server
            .read()
            .await
            .send_command(
                new_message.channel_id,
                new_message.author.id,
                new_message.content.clone(),
            )
            .await;
        for attachment in &new_message.attachments {
            let filedata = attachment.download().await?;
            let _ = data.server
                .read()
                .await
                .send_file(
                    new_message.channel_id,
                    new_message.author.id,
                    attachment.filename.clone(),
                    filedata,
                )
                .await;
        }
    }
    Ok(())
}
mod embedbuilder;
mod healthcheck;
mod messages;
mod server;
mod test;
mod helper;

use color_eyre::eyre;
use log::error;
use std::env;
use std::sync::Arc;
use color_eyre::eyre::eyre;

use crate::server::{ClientList, run_server, send_command, send_file, send_stats};

use crate::healthcheck::healthcheck;
use tokio::task;

use poise::{async_trait, Framework, serenity_prelude as serenity};
use serenity::all::{ChannelId, Context, EventHandler, GatewayIntents, Message, Ready};
use serenity::Client;
use tokio::sync::RwLock;

struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;


struct Handler {
    healthcheckchannel: ChannelId,
    clients: ClientList,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, new_message: Message) {
        // Check for statistics messages
        if new_message.channel_id == self.healthcheckchannel && new_message.content == "/stats" {
            send_stats(new_message.channel_id, ctx.clone(), self.clients.clone())
                .await;
        }

        // Check for health check message.
        if new_message.author.id == ctx.cache.current_user().id {
            if new_message.channel_id == self.healthcheckchannel {
                if new_message.embeds.len() != 1 {
                    return;
                }
                let embed1 = new_message.embeds.first().unwrap();
                if embed1.title.is_none() {
                    return;
                }
                let flag = embed1.title.as_ref().unwrap().clone();
                let _ = send_command(new_message.channel_id, new_message.author.id, flag, self.clients.clone())
                    .await;
                return;
            }
            return;
        }

        if new_message.guild_id.is_none() {
            return;
        }
        // Process all other messages as normal.
        let _ = send_command(
                new_message.channel_id,
                new_message.author.id,
                new_message.content,
                self.clients.clone()
            )
            .await;
        for attachment in new_message.attachments {
            let filedata = attachment.download().await.unwrap();
            let _ = send_file(
                    new_message.channel_id,
                    new_message.author.id,
                    attachment.filename,
                    filedata,
                    self.clients.clone()
                )
                .await;
        }
    }

    async fn ready(&self, _ctx: Context, _ready: Ready) {
        let ctx = Arc::new(_ctx);
        task::spawn(run_server(ctx, self.clients.clone()));
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    console_subscriber::init();

    for argument in env::args() {
        let result = match argument.to_lowercase().as_str() {
            "serve" => {
                serve().await
            }
            "healthcheck" => {
                healthcheck().await
            }
            &_ => {Ok(())}
        };
        result.unwrap();  // deliberately panic if we failed.

    }
    error!("Usage: TODO");
}


async fn serve() -> eyre::Result<()> {
    let framework: Framework<Data, Error> = Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let channelid: u64 = env::var("HEALTH_CHECK_CHANNEL_ID")
        .expect("channel id")
        .parse()?;

    let handler = Handler {
        healthcheckchannel: ChannelId::from(channelid),
        clients: Arc::new(RwLock::new(Vec::new())),
    };

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client: Client = Client::builder(token, intents)
        .event_handler(handler)
        .framework(framework)
        .await?;

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        return Err(eyre!("An error occurred while running the client: {:?}", why));
    }
    Ok(())
}

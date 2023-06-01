mod server;
mod test;

use async_std::sync::RwLock;
use serenity::client::{Context, EventHandler};
use serenity::Client;
use std::env;
use std::sync::Arc;

use crate::server::Server;
use serenity::async_trait;
use serenity::framework::standard::StandardFramework;

use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::GatewayIntents;
use tokio::task;

struct Handler {
    server: Arc<RwLock<Server>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, new_message: Message) {
        println!("{}", new_message.content);
        if new_message.is_own(ctx.cache) {
            return;
        }
        if new_message.is_private() {
            return;
        }
        self.server
            .read()
            .await
            .send_command(
                new_message.channel_id,
                new_message.author.id,
                new_message.content,
            )
            .await;
        for attachment in new_message.attachments {
            let filedata = attachment.download().await.unwrap();
            self.server
                .read()
                .await
                .send_file(
                    new_message.channel_id,
                    new_message.author.id,
                    attachment.filename,
                    filedata,
                )
                .await;
        }
    }

    async fn ready(&self, _ctx: Context, _ready: Ready) {
        let ctx = Arc::new(_ctx);
        task::spawn(run_server(ctx, self.server.clone()));
    }
}

async fn run_server(_ctx: Arc<Context>, server: Arc<RwLock<Server>>) {
    server.read().await.run(_ctx).await
}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new().configure(|c| c.prefix("~"));

    let handler = Handler {
        server: Arc::new(RwLock::new(Server::new())),
    };

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client: Client = Client::builder(token, intents)
        .event_handler(handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

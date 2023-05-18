mod server;

use serenity::client::{Context, EventHandler};
use serenity::Client;
use std::env;
use std::sync::Arc;

use crate::server::server;
use serenity::async_trait;
use serenity::framework::standard::StandardFramework;

use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::GatewayIntents;
use tokio::task;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, _new_message: Message) {
        println!("{}", _new_message.content);
    }

    async fn ready(&self, _ctx: Context, _ready: Ready) {
        let ctx = Arc::new(_ctx);
        task::spawn(server(ctx));
    }
}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new().configure(|c| c.prefix("~")); // set the bot's prefix to "~"

    env::set_var(
        "DISCORD_TOKEN",
        "NDgxMjc0NTU4MjM4OTQ5NDE1.GOg4Sf.ptkCm-QUW7cCI5gkg03SKvYPjbQDC_JbZV8umY",
    );

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client: Client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

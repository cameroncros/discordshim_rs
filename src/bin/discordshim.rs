use discordshim::discord::serve;
use poise::serenity_prelude as serenity;

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    console_subscriber::init();

    serve().await.unwrap();
}

// async fn serve() -> eyre::Result<()> {
//     let framework: Framework<Data, Error> = Framework::builder()
//         .options(poise::FrameworkOptions {
//             commands: vec![],
//             ..Default::default()
//         })
//         .setup(|ctx, _ready, framework| {
//             Box::pin(async move {
//                 poise::builtins::register_globally(ctx, &framework.options().commands).await?;
//                 Ok(Data {})
//             })
//         })
//         .build();
//
//     let channelid: u64 = env::var("HEALTH_CHECK_CHANNEL_ID")
//         .expect("channel id")
//         .parse()?;
//
//     let handler = Handler {
//         healthcheckchannel: ChannelId::from(channelid),
//         server: Arc::new(RwLock::new(Server::new())),
//     };
//
//     // Login with a bot token from the environment
//     let token = env::var("DISCORD_TOKEN").expect("token");
//     let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
//     let mut client: Client = Client::builder(token, intents)
//         .event_handler(handler)
//         .framework(framework)
//         .await?;
//
//     // start listening for events by starting a single shard
//     if let Err(why) = client.start().await {
//         return Err(eyre!(
//             "An error occurred while running the client: {:?}",
//             why
//         ));
//     }
//     Ok(())
// }

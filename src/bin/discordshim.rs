use discordshim::discord::serve;

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    console_subscriber::init();

    serve().await.unwrap();
}

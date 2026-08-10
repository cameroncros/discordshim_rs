use async_std::net::TcpStream;
use byteorder::{ByteOrder, LittleEndian};
use color_eyre::{eyre, eyre::eyre};
use discordshim::messages::{EmbedContent, Response, Settings, response::Field};
use futures::AsyncWriteExt;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::FullEvent;
use prost::Message;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::select;
use tokio::time::sleep;

async fn sender(flag: String) -> eyre::Result<()> {
    loop {
        if READY.load(Ordering::Acquire) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await
    }

    let mut client = TcpStream::connect("127.0.0.1:23416").await?;

    let channel_id: u64 = env::var("HEALTH_CHECK_CHANNEL_ID")
        .expect("channel id")
        .parse()?;
    // Send settings
    {
        let response = Response {
            field: Some(Field::Settings(Settings {
                channel_id,
                ..Default::default()
            })),
        };

        let bytes = response.encode_to_vec();
        send_data(&mut client, bytes).await?;
    }
    // Send flag
    {
        let response = Response {
            field: Some(Field::Embed(EmbedContent {
                title: flag.clone(),
                ..Default::default()
            })),
        };

        let bytes = response.encode_to_vec();
        send_data(&mut client, bytes).await?;
    }
    Ok(())
}

static FOUND: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);

async fn wait_for_flag() -> eyre::Result<()> {
    for _ in 0..50 {
        if FOUND.load(Ordering::Acquire) {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }

    Err(eyre!("Didnt see flag."))
}

struct Data {
    flag: String,
}

async fn receiver(flag: String) -> eyre::Result<()> {
    let health_check_token: String = env::var("HEALTH_CHECK_TOKEN").expect("health check token");
    let _channel_id: u64 = env::var("HEALTH_CHECK_CHANNEL_ID")
        .expect("channel id")
        .parse()?;

    async fn event_handler(event: &FullEvent, data: &Data) -> eyre::Result<()> {
        match event {
            FullEvent::Ready { data_about_bot: _ } => READY.store(true, Ordering::Release),
            FullEvent::Message { new_message } => {
                if new_message.embeds.len() == 1
                    && let Some(title) = new_message.embeds[0].title.clone()
                    && title == data.flag
                {
                    FOUND.store(true, Ordering::Release);
                }
            }
            _ => {}
        }
        Ok(())
    }

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            event_handler: |_ctx, event, _framework, data| {
                Box::pin(async move { event_handler(event, data).await })
            },
            ..Default::default()
        })
        .setup(|_ctx, _ready, _framework| Box::pin(async move { Ok(Data { flag: flag.clone() }) }))
        .build();

    let intents = serenity::GatewayIntents::privileged()
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::DIRECT_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT;
    let mut client = serenity::ClientBuilder::new(health_check_token, intents)
        .framework(framework)
        .await?;

    select! {
        _ = client.start() => Err(eyre!("Client failed"))?,
        res = wait_for_flag() => res?,
    }

    Ok(())
}

#[tokio::main]
pub async fn main() -> eyre::Result<()> {
    let flag = uuid::Uuid::new_v4().to_string();

    let (first, second) = tokio::join!(sender(flag.clone()), receiver(flag));
    first?;
    second?;
    Ok(())
}

async fn send_data(tcpstream: &mut TcpStream, data: Vec<u8>) -> eyre::Result<()> {
    let length = data.len() as u32;
    let length_buf = &mut [0u8; 4];
    LittleEndian::write_u32(length_buf, length);

    tcpstream.write_all(length_buf).await?;
    tcpstream.write_all(&data).await?;
    Ok(())
}

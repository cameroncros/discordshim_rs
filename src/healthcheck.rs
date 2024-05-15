use std::env;

use async_std::io::ReadExt;
use async_std::net::TcpStream;
use byteorder::{ByteOrder, LittleEndian};
use color_eyre::eyre;
use color_eyre::eyre::eyre;
use futures::AsyncWriteExt;
use protobuf::Message;

use crate::messages;

pub async fn healthcheck() -> eyre::Result<()> {
    let mut client = TcpStream::connect("127.0.0.1:23416").await.unwrap();
    let channel_id: u64 = env::var("HEALTH_CHECK_CHANNEL_ID")
        .expect("channel id")
        .parse()?;
    let flag = uuid::Uuid::new_v4().to_string();

    // Send settings
    {
        let mut response = messages::Response::new();
        let mut settings = messages::Settings {
            channel_id,
            ..Default::default()
        };
        settings.channel_id = channel_id;
        response.set_settings(settings);

        let bytes = response.write_to_bytes()?;
        send_data(&mut client, bytes).await?;
    }
    // Send flag
    {
        let mut response = messages::Response::new();
        let message = messages::EmbedContent {
            title: flag.clone(),
            ..Default::default()
        };
        response.set_embed(message);

        let bytes = response.write_to_bytes()?;
        send_data(&mut client, bytes).await?;
    }

    // Read up to 5 responses
    for _ in 0..5 {
        let length_buf = &mut [0u8; 4];
        client.read_exact(length_buf).await?;
        let length = LittleEndian::read_u32(length_buf) as usize;

        let mut buf = vec![0u8; length];
        client.read_exact(&mut buf).await?;

        let request = messages::Request::parse_from_bytes(buf.as_slice())?;
        if request.command() == flag {
            return Ok(()); // Success
        }
    }
    Err(eyre!("Failed healthcheck"))
}

async fn send_data(tcpstream: &mut TcpStream, data: Vec<u8>) -> eyre::Result<()> {
    let length = data.len() as u32;
    let length_buf = &mut [0u8; 4];
    LittleEndian::write_u32(length_buf, length);

    tcpstream.write_all(length_buf).await?;
    tcpstream.write_all(&data).await?;
    Ok(())
}

use std::env;

use byteorder::{ByteOrder, LittleEndian};
use color_eyre::eyre;
use color_eyre::eyre::eyre;
use protobuf::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::messages;
use crate::messages::Response;

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

        send_response(&mut client, response).await?;
    }
    // Send flag
    {
        let mut response = Response::new();
        let message = messages::EmbedContent {
            title: flag.clone(),
            ..Default::default()
        };
        response.set_embed(message);

        send_response(&mut client, response).await?;
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

pub async fn send_response(tcpstream: &mut (impl AsyncWriteExt + Unpin), msg: Response) -> eyre::Result<()> {
    let mut data = vec![];
    msg.write_to_vec(&mut data)?;

    let length = data.len() as u32;
    let length_buf = &mut [0u8; 4];
    LittleEndian::write_u32(length_buf, length);

    tcpstream.write_all(length_buf).await?;
    tcpstream.write_all(&data).await?;
    Ok(())
}
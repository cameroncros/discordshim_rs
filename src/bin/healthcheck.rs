use std::env;

use async_std::{io::ReadExt, net::TcpStream};
use byteorder::{ByteOrder, LittleEndian};
use color_eyre::{eyre, eyre::eyre};
use discordshim::messages::{
    EmbedContent,
    Request,
    Response,
    Settings,
    request::Message::Command,
    response::Field,
};
use futures::AsyncWriteExt;
use prost::Message;

#[tokio::main]
pub async fn main() -> eyre::Result<()> {
    let mut client = TcpStream::connect("127.0.0.1:23416").await.unwrap();
    let channel_id: u64 = env::var("HEALTH_CHECK_CHANNEL_ID")
        .expect("channel id")
        .parse()?;
    let flag = uuid::Uuid::new_v4().to_string();

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

    // Read up to 5 responses
    for _ in 0..5 {
        let length_buf = &mut [0u8; 4];
        client.read_exact(length_buf).await?;
        let length = LittleEndian::read_u32(length_buf) as usize;

        let mut buf = vec![0u8; length];
        client.read_exact(&mut buf).await?;

        let request = Request::decode(buf.as_slice())?;
        if let Some(Command(command)) = request.message
            && command == flag
        {
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

use byteorder::{ByteOrder, LittleEndian};
use color_eyre::eyre;
use discordshim::messages::response::Field;
use discordshim::messages::response::Field::Embed;
use discordshim::messages::{EmbedContent, Request, Response, Settings};
use log::debug;
use prost::Message;
use tokio::net::TcpStream;
use tokio::net::tcp::{ReadHalf, WriteHalf};

use clap::Parser;
use clap_derive::Parser;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "467700763775205396")]
    channel_id: u64,
    #[arg(long, default_value = "opdrshim.uk")]
    address: String,
    #[arg(long, default_value = "23416")]
    port: u16,
}

async fn send_msg(client: &mut WriteHalf<'_>, message: Response) -> eyre::Result<()> {
    let message_bytes = message.encode_to_vec();

    let length = u32::try_from(message_bytes.len())?;
    let length_buf = &mut [0u8; 4];
    LittleEndian::write_u32(length_buf, length);

    client.write_all(length_buf).await?;
    client.write_all(&message_bytes).await?;

    Ok(())
}

async fn recv_msg(client: &mut ReadHalf<'_>) -> eyre::Result<Request> {
    let length_buf = &mut [0u8; 4];
    client.read_exact(length_buf).await?;
    let length = LittleEndian::read_u32(length_buf) as usize;
    debug!("Incoming response, {length} bytes long.");

    let mut buf = vec![0u8; length];
    client.read_exact(&mut buf).await?;

    Ok(Request::decode(buf.as_slice())?)
}

async fn sender(client: &mut WriteHalf<'_>) -> eyre::Result<()> {
    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut input = String::new();

    loop {
        stdin
            .read_line(&mut input)
            .await
            .expect("Failed to read from STDIN");
        let text = input.trim().to_string();
        if text.is_empty() {
            input.clear();
            continue;
        }

        let embed_content = EmbedContent {
            title: "".to_string(),
            description: text,
            author: "".to_string(),
            color: 0,
            snapshot: None,
            textfield: vec![],
        };

        let response = Response {
            field: Some(Embed(embed_content)),
        };

        send_msg(client, response).await?;

        input.clear();
    }
}

async fn receiver(client: &mut ReadHalf<'_>) -> eyre::Result<()> {
    loop {
        let message = match recv_msg(client).await {
            Ok(m) => m,
            Err(e) => {
                println!("Failed to recv: {e}");
                return Err(e);
            }
        };
        println!("{message:?}");
    }
}

#[tokio::main(worker_threads = 2)]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let mut client = TcpStream::connect(format!("{}:{}", args.address, args.port)).await?;
    let (mut recv, mut send) = client.split();

    let handshake = Response {
        field: Some(Field::Settings(Settings {
            channel_id: args.channel_id,
            presence_enabled: false,
            cycle_time: 0,
            command_prefix: "DOESNT MATTER".to_string(),
        })),
    };
    send_msg(&mut send, handshake).await?;

    tokio::select! {
        _ = sender(&mut send) => {},
        _ = receiver(&mut recv) => {},
    };

    Ok(())
}

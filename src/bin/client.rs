use byteorder::{ByteOrder, LittleEndian};
use color_eyre::eyre;
use discordshim::messages::response::Field;
use discordshim::messages::{Request, Response, Settings};
use log::debug;
use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::task::yield_now;

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

async fn sender(_client: &mut WriteHalf<'_>) -> eyre::Result<()> {
    loop {
        yield_now().await
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

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let mut client = TcpStream::connect("192.168.1.100:53416").await?;
    let (mut recv, mut send) = client.split();

    let handshake = Response {
        field: Some(Field::Settings(Settings {
            channel_id: 467700763775205396,
            presence_enabled: false,
            cycle_time: 0,
            command_prefix: "!".to_string(),
        })),
    };
    send_msg(&mut send, handshake).await?;

    receiver(&mut recv).await?;

    // tokio::select!(
    //     _ = sender(&mut send) => {},
    //     _ = receiver(&mut recv) => {},
    // );

    Ok(())
}

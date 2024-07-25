
use byteorder::{ByteOrder, LittleEndian};
use color_eyre::eyre;
use log::debug;
use protobuf::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::messages::Response;
use crate::messages::Request;

pub async fn send_request(tcpstream:  &mut (impl AsyncWriteExt + Unpin), msg: Request) -> eyre::Result<()> {
    let mut data = vec![];
    msg.write_to_vec(&mut data)?;
    
    let length = data.len() as u32;
    let length_buf = &mut [0u8; 4];
    LittleEndian::write_u32(length_buf, length);

    tcpstream.write_all(length_buf).await?;
    tcpstream.write_all(&data).await?;
    Ok(())
}

pub async fn receive_msg(tcpstream:  &mut (impl AsyncReadExt + Unpin)) -> eyre::Result<Response> {
    let length_buf = &mut [0u8; 4];
    tcpstream.read_exact(length_buf).await?;
    let length = LittleEndian::read_u32(length_buf) as usize;
    debug!("Incoming response, {length} bytes long.");

    let mut buf = vec![0u8; length];
    tcpstream.read_exact(&mut buf).await?;

    Ok(Response::parse_from_bytes(buf.as_slice())?)
}
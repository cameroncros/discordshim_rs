use crate::server::discord_shim::Response;
use async_std::io::ReadExt;
use async_std::net::TcpListener;
use async_std::net::TcpStream;
use byteorder::{ByteOrder, LittleEndian};
use futures::stream::StreamExt;
use prost::Message;
use serenity::client::Context;
use serenity::model::id::ChannelId;
use serenity::model::prelude::Activity;
use serenity::model::prelude::OnlineStatus;
use std::io::Cursor;
use std::sync::Arc;

pub mod discord_shim {
    include!(concat!(env!("OUT_DIR"), "/discord_shim.rs"));
}

pub(crate) async fn server(ctx: Arc<Context>) {
    let listener = TcpListener::bind("0.0.0.0:12345")
        .await
        .expect("Failed to bind");
    listener
        .incoming()
        .for_each_concurrent(None, |tcpstream| {
            let ctx2 = ctx.clone();
            async move {
                let tcpstream = tcpstream.unwrap();
                let f = ctx2.clone();
                connection_loop(tcpstream, f).await.unwrap();
            }
        })
        .await;
}

async fn connection_loop(mut stream: TcpStream, ctx: Arc<Context>) -> Result<(), ()> {
    loop {
        let length_buf = &mut [0u8; 4];
        stream.read_exact(length_buf).await.unwrap();
        let length = LittleEndian::read_u32(length_buf) as usize;

        let mut buf = vec![0u8; length];
        stream.read_exact(&mut buf).await.unwrap();

        let response = Response::decode(&mut Cursor::new(buf)).unwrap();

        handle_task(response, ctx.clone()).await;
    }
}

async fn handle_task(response: Response, ctx: Arc<Context>) {
    let channel = ChannelId(467700763775205396);
    if response.file.is_some() {
        channel
            .send_message(&ctx, |m| {
                m.embed(|e| {
                    e.title("System Resource Load");
                    e.field("CPU Load Average", format!("{:.2}%", 10.0), false);
                    e.field(
                        "Memory Usage",
                        format!("{:.2} MB Free out of {:.2} MB", 1000.0, 1000.0),
                        false,
                    );
                    e
                })
            })
            .await
            .unwrap();
    }
    if response.presence.is_some() {
        let activity = Activity::playing(response.presence.unwrap().presence);
        ctx.shard.set_presence(Some(activity), OnlineStatus::Online);
    }
}

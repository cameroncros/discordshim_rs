#[cfg(test)]
mod tests {
    use crate::test::tests::discord_shim::{ProtoFile, Response, Settings, TextField};
    use byteorder::{ByteOrder, LittleEndian};
    use prost::Message;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};

    static CHANNEL_ID: u64 = 467700763775205396;

    pub mod discord_shim {
        include!(concat!(env!("OUT_DIR"), "/discord_shim.rs"));
    }

    fn send_message(stream: &mut TcpStream, response: &mut Response) {
        let msg = response.encode_to_vec();
        let length = msg.len() as u32;
        let length_buf = &mut [0u8; 4];
        LittleEndian::write_u32(length_buf, length);

        stream.write_all(length_buf).unwrap();
        stream.write_all(msg.as_slice()).unwrap();
    }

    fn recv_message(stream: &mut TcpStream) -> discord_shim::Request {
        let length_buf = &mut [0u8; 4];
        stream.read_exact(length_buf).unwrap();
        let length = LittleEndian::read_u32(length_buf);

        let mut buf = vec![0u8; length as usize];
        stream.read_exact(&mut buf).unwrap();

        return discord_shim::Request::decode(buf.as_slice()).unwrap();
    }

    fn get_snapshot() -> ProtoFile {
        let mut file = ProtoFile::default();
        file.filename = "filename.png".to_string();

        let filedata;
        match std::fs::read("test_data/test_pattern.png") {
            Ok(bytes) => filedata = bytes,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    eprintln!("please run again with appropriate permissions.");
                }
                panic!("{}", e);
            }
        }
        file.data = filedata;
        return file;
    }

    #[test]
    fn test_send_file() {
        let mut stream = TcpStream::connect("localhost:12345").unwrap();
        println!("Successfully connected to server in port 12345");

        let mut response = Response::default();
        let mut settings = Settings::default();
        settings.channel_id = CHANNEL_ID;
        response.settings = Some(settings);

        send_message(&mut stream, &mut response);

        let mut response = Response::default();
        let snapshot = get_snapshot();
        response.file = Some(snapshot);

        send_message(&mut stream, &mut response);

        stream.shutdown(Shutdown::Both).unwrap();
        println!("Terminated.");
    }

    #[test]
    fn test_send_embed() {
        let mut stream = TcpStream::connect("localhost:12345").unwrap();
        println!("Successfully connected to server in port 12345");

        let mut response = Response::default();
        let mut settings = Settings::default();
        settings.channel_id = CHANNEL_ID;
        response.settings = Some(settings);

        send_message(&mut stream, &mut response);

        let mut response = Response::default();
        let mut discord_embed = discord_shim::EmbedContent::default();
        discord_embed.title = "Title".to_string();
        discord_embed.description = "Description".to_string();
        discord_embed.author = "Author".to_string();
        discord_embed.color = 0x123456;
        let snapshot = get_snapshot();
        discord_embed.snapshot = Some(snapshot);
        for i in 0..50 {
            let mut field = TextField::default();
            field.title = i.to_string();
            field.text = "".to_string();
            field.inline = true;
            discord_embed.textfield.insert(0, field);
        }
        response.embed = Some(discord_embed);

        send_message(&mut stream, &mut response);

        stream.shutdown(Shutdown::Both).unwrap();
        println!("Terminated.");
    }

    #[test]
    fn test_recv_message() {
        let mut stream = TcpStream::connect("localhost:12345").unwrap();
        println!("Successfully connected to server in port 12345");

        let mut response = Response::default();
        let mut settings = Settings::default();
        settings.channel_id = CHANNEL_ID;
        response.settings = Some(settings);

        send_message(&mut stream, &mut response);
        let mut seen_file = false;
        let mut seen_command = false;
        loop {
            let request = recv_message(&mut stream);
            if request.file.is_some() {
                let file = request.file.clone().unwrap();
                println!(
                    "Received file: [{}], size: [{}]",
                    file.filename,
                    file.data.len()
                );
                assert_ne!(request.user, 0);
                seen_file = true;
            }
            if !request.command.is_empty() {
                println!("Received command: [{}]", request.command);
                assert_ne!(request.user, 0);
                seen_command = true;
            }
            if seen_file && seen_command {
                break;
            }
        }
    }
}

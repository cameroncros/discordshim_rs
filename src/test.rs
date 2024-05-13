#[cfg(test)]
mod tests {
    use crate::embedbuilder::{
        build_embeds, split_file, DISCORD_MAX_AUTHOR, DISCORD_MAX_DESCRIPTION, DISCORD_MAX_FIELDS,
        DISCORD_MAX_TITLE, DISCORD_MAX_VALUE, ONE_MEGABYTE,
    };
    use crate::messages;
    use crate::messages::{EmbedContent, Response, Settings, TextField};
    use byteorder::{ByteOrder, LittleEndian};
    use protobuf::{Message, MessageField};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};

    static CHANNEL_ID: u64 = 467700763775205396;

    fn send_message(stream: &mut TcpStream, response: &mut Response) {
        let msg = response.write_to_bytes().unwrap();
        let length = msg.len() as u32;
        let length_buf = &mut [0u8; 4];
        LittleEndian::write_u32(length_buf, length);

        stream.write_all(length_buf).unwrap();
        stream.write_all(msg.as_slice()).unwrap();
    }

    fn recv_message(stream: &mut TcpStream) -> messages::Request {
        let length_buf = &mut [0u8; 4];
        stream.read_exact(length_buf).unwrap();
        let length = LittleEndian::read_u32(length_buf);

        let mut buf = vec![0u8; length as usize];
        stream.read_exact(&mut buf).unwrap();

        return messages::Request::parse_from_bytes(buf.as_slice()).unwrap();
    }

    fn get_snapshot() -> messages::ProtoFile {
        let mut file = messages::ProtoFile::default();
        file.filename = "filename.png".to_string();

        
        let filedata = match std::fs::read("test_data/test_pattern.png") {
            Ok(bytes) => bytes,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    eprintln!("please run again with appropriate permissions.");
                }
                panic!("{}", e);
            }
        };
        file.data = filedata;
        file
    }

    #[ignore]
    #[test]
    fn test_send_file() {
        let mut stream = TcpStream::connect("localhost:12345").unwrap();
        println!("Successfully connected to server in port 12345");

        let mut response = Response::default();
        let mut settings = Settings::default();
        settings.channel_id = CHANNEL_ID;
        response.field = Some(messages::response::Field::Settings(settings));

        send_message(&mut stream, &mut response);

        let mut response = Response::default();
        let snapshot = get_snapshot();
        response.field = Some(messages::response::Field::File(snapshot));

        send_message(&mut stream, &mut response);

        stream.shutdown(Shutdown::Both).unwrap();
        println!("Terminated.");
    }

    #[ignore]
    #[test]
    fn test_send_embed() {
        let mut stream = TcpStream::connect("localhost:12345").unwrap();
        println!("Successfully connected to server in port 12345");

        let mut response = Response::default();
        let mut settings = Settings::default();
        settings.channel_id = CHANNEL_ID;
        response.field = Some(messages::response::Field::Settings(settings));

        send_message(&mut stream, &mut response);

        let mut response = Response::default();
        let mut discord_embed = messages::EmbedContent::default();
        discord_embed.title = "Title".to_string();
        discord_embed.description = "Description".to_string();
        discord_embed.author = "Author".to_string();
        discord_embed.color = 0x123456;
        let snapshot = get_snapshot();
        discord_embed.snapshot = MessageField::some(snapshot);
        for i in 0..50 {
            let field = TextField {
                title: i.to_string(),
                text:"".to_string(),
                inline: true,
                ..Default::default()
            };
            discord_embed.textfield.insert(0, field);
            
        }
        response.field = Some(messages::response::Field::Embed(discord_embed));

        send_message(&mut stream, &mut response);

        stream.shutdown(Shutdown::Both).unwrap();
        println!("Terminated.");
    }

    #[ignore]
    #[test]
    fn test_recv_message() {
        let mut stream = TcpStream::connect("localhost:12345").unwrap();
        println!("Successfully connected to server in port 12345");

        let mut response = Response::default();
        let settings = Settings {
            channel_id: CHANNEL_ID,
            ..Default::default()
        };
        response.field = Some(messages::response::Field::Settings(settings));

        send_message(&mut stream, &mut response);
        let mut seen_file = false;
        let mut seen_command = false;
        loop {
            let request = recv_message(&mut stream);
            match request.message {
                None => {}
                Some(messages::request::Message::File(file)) => {
                    println!(
                        "Received file: [{}], size: [{}]",
                        file.filename,
                        file.data.len()
                    );
                    assert_ne!(request.user, 0);
                    seen_file = true;
                }
                Some(messages::request::Message::Command(command)) => {
                    println!("Received command: [{}]", command);
                    assert_ne!(request.user, 0);
                    seen_command = true;
                }
            }
            if seen_file && seen_command {
                break;
            }
        }
    }

    #[test]
    fn test_split_file_small_file() {
        let attachments = split_file("filename".to_string(), "filedata".as_bytes());
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].0, "filename");
    }

    #[test]
    fn test_split_file_large_file() {
        let mut file = File::open("/dev/urandom").unwrap();
        let mut filedata = vec![0u8; 7 * ONE_MEGABYTE];
        file.read_exact(&mut filedata).unwrap();
        let attachments = split_file("filename".to_string(), &filedata);
        assert_eq!(attachments.len(), 8);
        assert_eq!(attachments[0].0, "filename.zip.000");
        assert_eq!(attachments[1].0, "filename.zip.001");
        assert_eq!(attachments[2].0, "filename.zip.002");
        assert_eq!(attachments[3].0, "filename.zip.003");
        assert_eq!(attachments[4].0, "filename.zip.004");
        assert_eq!(attachments[5].0, "filename.zip.005");
        assert_eq!(attachments[6].0, "filename.zip.006");
        assert_eq!(attachments[7].0, "filename.zip.007");
    }

    #[test]
    fn test_build_embeds_min() {
        let textfields = vec![TextField {
            title: str::repeat("d", DISCORD_MAX_TITLE),
            text: str::repeat("e", DISCORD_MAX_VALUE),
            inline: false,
            special_fields: Default::default(),
        }];
        let ec = EmbedContent {
            title: str::repeat("a", DISCORD_MAX_TITLE),
            description: str::repeat("b", DISCORD_MAX_DESCRIPTION),
            author: str::repeat("c", DISCORD_MAX_AUTHOR),
            color: 0,
            snapshot: Default::default(),
            textfield: textfields,
            special_fields: Default::default(),
        };

        let embeds = build_embeds(ec);
        assert_eq!(1, embeds.len());
    }

    #[test]
    fn test_build_embeds_max() {
        let mut textfields = vec![];
        for _i in 0..(DISCORD_MAX_FIELDS + 1) {
            textfields.push(TextField {
                title: str::repeat("d", DISCORD_MAX_TITLE),
                text: str::repeat("e", DISCORD_MAX_VALUE),
                inline: false,
                special_fields: Default::default(),
            });
        }
        let ec = EmbedContent {
            title: str::repeat("c", DISCORD_MAX_TITLE),
            description: str::repeat("d", DISCORD_MAX_DESCRIPTION),
            author: str::repeat("e", DISCORD_MAX_AUTHOR),
            color: 0,
            snapshot: Default::default(),
            textfield: textfields,
            special_fields: Default::default(),
        };

        let embeds = build_embeds(ec.clone());
        assert_eq!(8, embeds.len());

        assert_eq!(ec.title, embeds[0].title);
        assert_eq!(ec.description, embeds[0].description);
        let mut num_fields = embeds[0].textfield.len();
        for embed in &embeds[1..] {
            assert_eq!("", embed.title);
            assert_eq!("\u{200b}", embed.description);
            num_fields += embed.textfield.len();
        }

        assert_eq!(num_fields, DISCORD_MAX_FIELDS + 1);
    }
}

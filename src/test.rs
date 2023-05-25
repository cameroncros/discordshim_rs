#[cfg(test)]
mod tests {
    use crate::test::tests::discord_shim::{ProtoFile, Response, Settings};
    use byteorder::{ByteOrder, LittleEndian};
    use prost::Message;
    use std::io::Write;
    use std::net::{Shutdown, TcpStream};

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

    #[test]
    fn it_file_upload() {
        let mut stream = TcpStream::connect("localhost:12345").unwrap();
        println!("Successfully connected to server in port 12345");

        let mut response = Response::default();
        let mut settings = Settings::default();
        settings.channel_id = 467700763775205396;
        response.settings = Some(settings);

        send_message(&mut stream, &mut response);

        let mut file = ProtoFile::default();
        file.filename = "filename.png".to_string();
        file.data = Vec::from("Hello World".as_bytes());
        response.file = Some(file);

        stream.shutdown(Shutdown::Both).unwrap();
        println!("Terminated.");
    }
}

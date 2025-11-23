use crate::messages::TextField;
use crate::messages::EmbedContent;
use std::borrow::Cow;
use std::io::{Cursor, Write};

use serenity::all::CreateAttachment;
use zip::write::SimpleFileOptions;

pub const ONE_MEGABYTE: usize = 1024 * 1024;
pub const DISCORD_MAX_ATTACHMENT_SIZE: usize = 5 * ONE_MEGABYTE;

pub const DISCORD_MAX_TITLE: usize = 256;
pub const DISCORD_MAX_DESCRIPTION: usize = 4096;
pub const DISCORD_MAX_FIELDS: usize = 25;
pub const DISCORD_MAX_VALUE: usize = 1024;
//pub const DISCORD_MAX_FOOTER: usize = 2048;
pub const DISCORD_MAX_AUTHOR: usize = 256;
pub const DISCORD_MAX_EMBED_TOTAL: usize = 6000;

fn truncate(string: String, length: usize) -> String {
    if string.len() > length {
        return string[0..length].to_string();
    }
    string
}

pub(crate) fn build_embeds(embed_content: EmbedContent) -> Vec<EmbedContent> {
    let mut embeds = vec![];
    let mut first = EmbedContent::default();
    let mut total_chars;
    first.title = truncate(embed_content.title, DISCORD_MAX_TITLE);
    first.description = if !embed_content.description.is_empty() {
        truncate(embed_content.description, DISCORD_MAX_DESCRIPTION)
    } else {
        "\u{200b}".to_string()
    };
    first.snapshot = embed_content.snapshot;

    let author = truncate(embed_content.author, DISCORD_MAX_AUTHOR);
    first.author.clone_from(&author);
    first.color = embed_content.color;

    total_chars = first.title.len() + first.description.len() + first.author.len();

    let mut last = first;

    for field in embed_content.textfield {
        let mut trimmed_field = TextField::default();
        let title = truncate(field.title, DISCORD_MAX_TITLE);
        let text = truncate(field.text, DISCORD_MAX_VALUE);

        trimmed_field.title.clone_from(&title);
        trimmed_field.text.clone_from(&text);
        trimmed_field.inline = field.inline;

        let next_size = total_chars + trimmed_field.title.len() + trimmed_field.text.len();
        if last.textfield.len() >= DISCORD_MAX_FIELDS || next_size > DISCORD_MAX_EMBED_TOTAL {
            embeds.push(last);
            last = EmbedContent::default();
            last.description = "\u{200b}".to_string();
            last.author.clone_from(&author);
            last.color = embed_content.color;
            total_chars = last.title.len() + last.description.len() + last.author.len();
        }

        last.textfield.push(trimmed_field);
        total_chars += title.len() + text.len();
    }

    embeds.push(last);
    embeds
}

pub(crate) fn split_file(filename: String, filedata: &[u8]) -> Vec<(String, CreateAttachment)> {
    return if filedata.len() < DISCORD_MAX_ATTACHMENT_SIZE {
        let mut attachments = vec![];
        let filename2 = filename.clone();
        attachments.push((
            filename,
            CreateAttachment::bytes(
                Cow::from(filedata),
                filename2,
            )
        ));
        attachments
    } else {
        let mut attachments = vec![];
        let bytes = Vec::new();
        let zipfile = Cursor::new(bytes);
        let mut zip = zip::ZipWriter::new(zipfile);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file(filename.clone(), options)
            .unwrap();
        zip.write_all(filedata).unwrap();
        let zipdata = zip.finish().unwrap().into_inner();

        let chunks = zipdata.chunks(ONE_MEGABYTE);
        for (i, chunk) in chunks.enumerate() {
            let zipfilename = format!("{}.zip.{:0>3}", filename, i);
            let mut data = vec![0u8; chunk.len()];
            data.copy_from_slice(chunk);
            attachments.push((
                zipfilename.clone(),
                CreateAttachment::bytes(
                    Cow::from(data),
                    zipfilename
                )
            ));
        }
        attachments
    };
}

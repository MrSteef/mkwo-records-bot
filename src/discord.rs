use std::env;
use std::str::FromStr;
use anyhow::Result;

use infer;
use mime::Mime;
use serenity::all::{Attachment, ChannelId};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

use crate::ocr;

pub struct Handler;
impl Handler {
    pub fn new() -> Self { Handler }
}

enum ValidationError {
    Chat(String),
    Console(String),
    Silent(&'static str),
}

fn validate_channel(msg: &Message) -> Result<(), ValidationError> {
    let channel_id = env::var("CHANNEL_ID")
        .map_err(|_| ValidationError::Console("Failed to get CHANNEL_ID env var".to_string()))?;
    let channel_id = ChannelId::from_str(&channel_id)
        .map_err(|_| ValidationError::Console("Invalid CHANNEL_ID format".to_string()))?;
    if msg.channel_id == channel_id {
        Ok(())
    } else {
        Err(ValidationError::Silent("Wrong channel"))
    }
}

fn validate_from_user(msg: &Message) -> Result<(), ValidationError> {
    if !msg.author.bot {
        Ok(())
    } else {
        Err(ValidationError::Silent("Author is bot"))
    }
}

fn validate_single_attachment(msg: &Message) -> Result<(), ValidationError> {
    if msg.attachments.len() == 1 {
        Ok(())
    } else {
        Err(ValidationError::Silent("Expected one attachment"))
    }
}

fn get_attachment(msg: &Message) -> Result<Attachment, ValidationError> {
    msg.attachments.get(0)
        .cloned()
        .ok_or(ValidationError::Chat("No attachment found".to_string()))
}

fn validate_filename_mime_type(att: &Attachment) -> Result<(), ValidationError> {
    let ct = att.content_type
        .as_ref()
        .ok_or(ValidationError::Chat("Missing content type".to_string()))?;
    let mime: Mime = ct.parse()
        .map_err(|_| ValidationError::Chat("Invalid mime type".to_string()))?;
    if mime.type_() == mime::IMAGE {
        Ok(())
    } else {
        Err(ValidationError::Chat("File is not an image".to_string()))
    }
}

async fn download_attachment(att: Attachment) -> Result<Vec<u8>, ValidationError> {
    att.download().await
        .map_err(|_| ValidationError::Chat("Download failed".to_string()))
}

fn validate_content_mime_type(data: &[u8]) -> Result<(), ValidationError> {
    let info = infer::get(data)
        .ok_or(ValidationError::Chat("Cannot infer file type".to_string()))?;
    if info.matcher_type() == infer::MatcherType::Image {
        Ok(())
    } else {
        Err(ValidationError::Chat("Content is not image".to_string()))
    }
}

async fn validate_all(msg: &Message) -> Result<Vec<u8>, ValidationError> {
    validate_channel(msg)?;
    validate_from_user(msg)?;
    validate_single_attachment(msg)?;
    let att = get_attachment(msg)?;
    validate_filename_mime_type(&att)?;
    let data = download_attachment(att).await?;
    validate_content_mime_type(&data)?;
    Ok(data)
}

async fn send_chat(ctx: &Context, msg: &Message, text: impl Into<String>) {
    let _ = msg.channel_id.say(&ctx.http, text.into()).await;
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let bytes = match validate_all(&msg).await {
            Ok(b) => b,
            Err(err) => {
                match err {
                    ValidationError::Chat(why) => send_chat(&ctx, &msg, why).await,
                    ValidationError::Console(e) => eprintln!("{}", e),
                    ValidationError::Silent(_)    => {},
                }
                return;
            }
        };

        match ocr::run_pipeline_from_bytes(&bytes, false) {
            Ok(text) => send_chat(&ctx, &msg, format!("Extracted text: {}", text)).await,
            Err(_) => send_chat(&ctx, &msg, "Sorry, I couldn’t process that image.").await,
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}
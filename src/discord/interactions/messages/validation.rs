use std::{env, str::FromStr};

use mime::Mime;
use serenity::all::{Attachment, ChannelId, Message};

pub enum ValidationOutcome {
    Ignore,
    UserError(&'static str),
    SystemError(&'static str),
}

pub async fn validate_all(msg: &Message) -> Result<Vec<u8>, ValidationOutcome> {
    validate_channel(msg)?;
    validate_from_user(msg)?;
    let att = get_single_attachment(msg)?;
    validate_filename_mime_type(&att)?;
    let data = download_attachment(att).await?;
    validate_content_mime_type(&data)?;
    Ok(data)
}

fn validate_channel(msg: &Message) -> Result<(), ValidationOutcome> {
    let channel_id = env::var("CHANNEL_ID")
        .map_err(|_| ValidationOutcome::SystemError("Failed to get CHANNEL_ID env var"))?;
    let channel_id = ChannelId::from_str(&channel_id)
        .map_err(|_| ValidationOutcome::SystemError("Invalid CHANNEL_ID format"))?;
    if msg.channel_id == channel_id {
        Ok(())
    } else {
        Err(ValidationOutcome::Ignore) 
    }
}

fn validate_from_user(msg: &Message) -> Result<(), ValidationOutcome> {
    if !msg.author.bot {
        Ok(())
    } else {
        Err(ValidationOutcome::Ignore)
    }
}

fn get_single_attachment(msg: &Message) -> Result<Attachment, ValidationOutcome> {
    if msg.attachments.len() != 1 {
        return Err(ValidationOutcome::Ignore);
    }

    msg.attachments
        .get(0)
        .cloned()
        .ok_or(ValidationOutcome::SystemError("Could not get attachment, even though it should exist"))
}

fn validate_filename_mime_type(att: &Attachment) -> Result<(), ValidationOutcome> {
    let ct = att
        .content_type
        .as_ref()
        .ok_or(ValidationOutcome::UserError("Missing content type"))?;
    let mime: Mime = ct
        .parse()
        .map_err(|_| ValidationOutcome::UserError("Invalid mime type"))?;
    if mime.type_() == mime::IMAGE {
        Ok(())
    } else {
        Err(ValidationOutcome::UserError("File is not an image"))
    }
}

async fn download_attachment(att: Attachment) -> Result<Vec<u8>, ValidationOutcome> {
    att.download()
        .await
        .map_err(|_| ValidationOutcome::UserError("Download failed"))
}

fn validate_content_mime_type(data: &[u8]) -> Result<(), ValidationOutcome> {
    let info =
        infer::get(data).ok_or(ValidationOutcome::UserError("Cannot infer file type"))?;
    if info.matcher_type() == infer::MatcherType::Image {
        Ok(())
    } else {
        Err(ValidationOutcome::UserError("Content is not image"))
    }
}
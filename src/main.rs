use std::env;
use std::str::FromStr;

use dotenv::dotenv;
use leptess::LepTess;
use mime::Mime;
use serenity::all::{Attachment, ChannelId};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

struct Handler;

enum ValidationError {
    // intended for mistakes that user might make, or unexpected behavior that the user should be informed about
    Chat(String), 
    // intended for unexpected behavior that is not relevant for the user
    Console(String),
    // intended for situations that should simply be ignored. str is for documentation
    Silent(&'static str),
}

fn validate_channel(msg: &Message) -> Result<(), ValidationError> {
    let channel_id = env::var("CHANNEL_ID")
        .map_err(|_| ValidationError::Console("Failed to get CHANNEL_ID environment variable".to_string()))?;
    let channel_id = ChannelId::from_str(&channel_id)
        .map_err(|_| ValidationError::Console("Failed to convert CHANNEL_ID to ChannelId".to_string()))?;
    (msg.channel_id == channel_id)
        .then(|| ())
        .ok_or(ValidationError::Silent("Message ChannelId did not match"))
}

fn validate_from_user(msg: &Message) -> Result<(), ValidationError> {
    (!msg.author.bot)
        .then(|| ())
        .ok_or(ValidationError::Silent("Message was sent by a bot"))
}

fn validate_single_attachment(msg: &Message) -> Result<(), ValidationError> {
    (msg.attachments.len() == 1)
        .then(|| ())
        .ok_or(ValidationError::Silent("Message did not have exactly one attachment"))
}

fn get_attachment(msg: &Message) -> Result<Attachment, ValidationError> {
    msg.attachments
        .get(0)
        .cloned()
        .ok_or(ValidationError::Chat("Failed to obtain attachment".to_string()))
}

fn validate_filename_mime_type(attachment: &Attachment) -> Result<(), ValidationError> {
    let media_type = attachment
        .content_type
        .as_ref()
        .ok_or(ValidationError::Chat("Failed to get attachment's content type".to_string()))?;
    let mime_type = Mime::from_str(&media_type)
        .map_err(|_| ValidationError::Chat("Failed to determine attachment's mime type".to_string()))?;
    (mime_type.type_() == mime::IMAGE)
        .then(|| ())
        .ok_or(ValidationError::Chat("Attached file is not an image".to_string()))
}

async fn download_attachment(attachment: Attachment) -> Result<Vec<u8>, ValidationError> {
    attachment
        .download()
        .await
        .map_err(|_| ValidationError::Chat("Failed to download attachment".to_string()))
}

fn validate_content_mime_type(file_contents: &[u8]) -> Result<(), ValidationError> {
    infer::get(file_contents)
        .ok_or(ValidationError::Chat("Failed to determine content mime type".to_string()))?
        .matcher_type()
        .eq(&infer::MatcherType::Image)
        .then(|| ())
        .ok_or(ValidationError::Chat("File content does not resemble an image".to_string()))
}

async fn validate_all(msg: &Message) -> Result<Vec<u8>, ValidationError> {
    validate_channel(msg)?;
    validate_from_user(msg)?;
    validate_single_attachment(msg)?;
    let attachment = get_attachment(msg)?;
    validate_filename_mime_type(&attachment)?;
    let file_contents = download_attachment(attachment).await?;
    validate_content_mime_type(&file_contents)?;

    Ok(file_contents)
}

async fn send_chat(ctx: &Context, msg: &Message, text: impl Into<String>) {
    let text: String = text.into();

    if let Err(why) = msg.channel_id.say(&ctx.http, text).await {
        eprintln!("Error sending message: {why:?}");
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let file = match validate_all(&msg).await {
            Ok(file) => file,
            Err(err) => {
                match err {
                    ValidationError::Chat(why) => send_chat(&ctx, &msg, why).await,
                    ValidationError::Console(why) => eprintln!("{why:?}"),
                    ValidationError::Silent(_) => {}
                }
                return;
            }
        };

        // text detection is still work in progress:
        let mut lt = LepTess::new(None, "eng").unwrap();
        lt.set_image_from_mem(&file).unwrap();
        let text = lt.get_utf8_text().unwrap();
        let times: Vec<&str> = text.matches(r"\d:\d{2}\.\d{3}").collect();

        if times.len() >= 1 {
            send_chat(&ctx, &msg, *times.get(1).unwrap()).await;
        } else {
            send_chat(&ctx, &msg, format!("Unable to extract times, raw text: {text:?}")).await;
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}

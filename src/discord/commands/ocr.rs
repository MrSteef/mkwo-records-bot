use serenity::all::{Context, Message};
use crate::discord::validation::{validate_all, ValidationError};
use crate::discord::util::send_chat;
use crate::ocr;

pub async fn handle_message(ctx: &Context, msg: &Message) {
    let bytes = match validate_all(msg).await {
        Ok(b) => b,
        Err(err) => {
            match err {
                ValidationError::Chat(why) => send_chat(ctx, msg, why).await,
                ValidationError::Console(e) => eprintln!("{}", e),
                ValidationError::Silent(_) => {}
            }
            return;
        }
    };

    match ocr::run_pipeline_from_bytes(&bytes, true, msg.id.get()) {
        Ok(text) => send_chat(ctx, msg, format!("Extracted text: {}", text)).await,
        Err(_) => send_chat(ctx, msg, "Sorry, I couldn’t process that image.").await,
    }
}

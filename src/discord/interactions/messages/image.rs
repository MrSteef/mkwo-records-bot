use serenity::all::{Context, EditMessage, Message};

use crate::{discord::{
    handler::Handler,
    interactions::messages::validation::{validate_all, ValidationOutcome},
    templates::record::record_embed,
}, sheets::records::record::Record};

pub enum OcrProcessOutcome<'a> {
    Success { record: Record<'a> },
    InvalidImage(String),
    PlayerMissing,
    TrackMissing,
    StorageFailure,
}

pub async fn handle_message(ctx: &Context, msg: &Message, handler: &Handler) {
    let bytes = match validate_all(msg).await {
        Ok(b) => b,
        Err(ValidationOutcome::Ignore) => return,
        Err(ValidationOutcome::SystemError(e)) => {
            eprintln!("{e}");
            return;
        }
        Err(ValidationOutcome::UserError(_)) => {
            // TODO: inform user
            return;
        }
    };

    let mut message = msg
        .reply(&ctx.http, "Please wait while the image is being processed")
        .await
        .unwrap();
    let result = process_ocr_message(msg, bytes, handler, &message).await;

    match result {
        OcrProcessOutcome::Success { record } => {
            let (embed, components) = record_embed(record, handler).await;

            let edit = EditMessage::new()
                .content("")
                .embed(embed)
                .components(components);
            message.edit(&ctx.http, edit).await.unwrap();
        }
        OcrProcessOutcome::InvalidImage(reason) => {
            let edit = EditMessage::new().content(reason);
            message.edit(&ctx.http, edit).await.unwrap();
        }
        OcrProcessOutcome::StorageFailure => {
            let edit = EditMessage::new().content("Failed to save record");
            message.edit(&ctx.http, edit).await.unwrap();
        }
        OcrProcessOutcome::PlayerMissing | OcrProcessOutcome::TrackMissing => {
            let edit = EditMessage::new()
                .content("Please select a track first using /play before uploading records.");
            message.edit(&ctx.http, edit).await.unwrap();
        }
    }
}

pub async fn process_ocr_message<'a>(
    msg: &Message,
    bytes: Vec<u8>,
    handler: &'a Handler,
    bot_msg: &Message,
) -> OcrProcessOutcome<'a> {
    let time = match crate::ocr::extract_time(&bytes).await {
        Ok(t) => t,
        Err(why) => {
            eprintln!("{why}");
            return OcrProcessOutcome::InvalidImage("Sorry, I couldn't process that image.".into());
        }
    };

    let players = handler
    .gsheet
    .players();

    let player = match players
        .get_by_user_id(msg.author.id.get())
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return OcrProcessOutcome::PlayerMissing,
        Err(_) => return OcrProcessOutcome::StorageFailure,
    };

    let track_name = match player.current_track.clone() {
        Some(name) => name,
        None => return OcrProcessOutcome::TrackMissing,
    };

    let created = handler
        .gsheet
        .records()
        .create(
            msg.id.get(),
            bot_msg.id.get(),
            msg.timestamp,
            msg.author.id.get(),
            track_name.clone(),
            time,
        )
        .await;

    let record = match created {
        Ok(record) => record,
        Err(why) => {
            eprintln!("storage failure: {}", why);
            return OcrProcessOutcome::StorageFailure
        },
    };

    OcrProcessOutcome::Success { record }
}

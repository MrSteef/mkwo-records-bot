use std::time::Duration;
use anyhow::{Result, anyhow};

use crate::discord::Handler;
use crate::discord::util::send_chat;
use crate::discord::validation::{ValidationError, validate_all};
use crate::ocr;
use serenity::all::{Context, EditMessage, Message};

pub async fn handle_message(ctx: &Context, msg: &Message, handler: &Handler) {
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

    let mut message = msg
        .reply(&ctx.http, "Please wait while the image is being processed")
        .await
        .unwrap();

    let time = match ocr::run_pipeline_from_bytes(&bytes, true, msg.id.get()) {
        Ok(time) => time,
        Err(_) => {
            let edit = EditMessage::new().content("Sorry, I couldn’t process that image.");
            message.edit(&ctx.http, edit).await.unwrap();
            return;
        }
    };

    let player = handler.gsheet.players().get_by_id(msg.author.id.get()).await;
    let player = match player {
        Ok(player) => player,
        Err(_) => {
            let edit = EditMessage::new().content("Failed to obtain selected track, please try again");
            message.edit(&ctx.http, edit).await.unwrap();
            return;
        },
    };

    // this track should be preventable if we first run players().create_if_not_exists()
    let player = match player {
        Some(player) => player,
        None => {
            let edit = EditMessage::new().content("Failed to obtain player data, please select a track first using /play");
            message.edit(&ctx.http, edit).await.unwrap();
            return;
        },
    };

    let track_name = match player.current_track {
        Some(track_name) => track_name,
        None => {
            let edit = EditMessage::new().content("Failed to obtain selected track, please select a track first using /play");
            message.edit(&ctx.http, edit).await.unwrap();
            return;
        },
    };

    let record = handler.gsheet.records().create_record(
        msg.id.get(),
        message.id.get(),
        msg.timestamp,
        msg.author.id.get(),
        track_name.clone(),
        parse_duration(&time).unwrap()
    ).await;

    println!("{:?}", record);

    let edit = EditMessage::new().content(format!("Added new time of {} on {}", time, track_name));

    message.edit(&ctx.http, edit).await.unwrap();
}

pub fn parse_duration(s: &str) -> Result<Duration> {
    // split into ["M", "SS.mmm"]
    let mut parts = s.split(':');
    let minutes_str = parts
        .next()
        .ok_or_else(|| anyhow!("Missing minutes in '{}'", s))?;
    let sec_ms_str = parts
        .next()
        .ok_or_else(|| anyhow!("Missing seconds in '{}'", s))?;
    // there should be no extra ':'
    if parts.next().is_some() {
        return Err(anyhow!("Unexpected extra ':' in '{}'", s));
    }

    // parse minutes
    let minutes: u64 = minutes_str
        .parse()
        .map_err(|e| anyhow!("Invalid minutes '{}' in '{}': {}", minutes_str, s, e))?;

    // split seconds and milliseconds
    let mut sec_parts = sec_ms_str.split('.');
    let seconds_str = sec_parts
        .next()
        .ok_or_else(|| anyhow!("Missing seconds before '.' in '{}'", s))?;
    let millis_str = sec_parts
        .next()
        .ok_or_else(|| anyhow!("Missing milliseconds after '.' in '{}'", s))?;
    // no extra '.'
    if sec_parts.next().is_some() {
        return Err(anyhow!("Unexpected extra '.' in seconds part of '{}'", s));
    }

    let seconds: u64 = seconds_str
        .parse()
        .map_err(|e| anyhow!("Invalid seconds '{}' in '{}': {}", seconds_str, s, e))?;
    let millis: u64 = match millis_str.len() {
        1..=3 => {
            // e.g. "5" -> "500", "45" -> "450", "123" -> "123"
            let scale = 10_u64.pow(3 - millis_str.len() as u32);
            let raw: u64 = millis_str
                .parse()
                .map_err(|e| anyhow!("Invalid millis '{}' in '{}': {}", millis_str, s, e))?;
            raw * scale
        }
        _ => return Err(anyhow!("Milliseconds must be 1–3 digits in '{}'", s)),
    };

    Ok(Duration::from_secs(minutes * 60 + seconds) + Duration::from_millis(millis))
}
use std::time::Duration;

use crate::discord::Handler;
use crate::discord::util::send_chat;
use crate::discord::validation::{ValidationError, validate_all};
use crate::ocr;
use serenity::all::{
    Colour, Context, CreateActionRow, CreateButton, CreateEmbed, EditMessage, Message,
};

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

    let time = match ocr::run_pipeline_from_bytes(&bytes, true, msg.id.get()).await {
        Ok(time) => time,
        Err(_) => {
            let edit = EditMessage::new().content("Sorry, I couldn’t process that image.");
            message.edit(&ctx.http, edit).await.unwrap();
            return;
        }
    };

    let player = handler
        .gsheet
        .players()
        .get_by_id(msg.author.id.get())
        .await;
    let player = match player {
        Ok(player) => player,
        Err(_) => {
            let edit =
                EditMessage::new().content("Failed to obtain selected track, please try again");
            message.edit(&ctx.http, edit).await.unwrap();
            return;
        }
    };

    // this check should be preventable if we first run players().create_if_not_exists()
    let player = match player {
        Some(player) => player,
        None => {
            let edit = EditMessage::new()
                .content("Failed to obtain player data, please select a track first using /play");
            message.edit(&ctx.http, edit).await.unwrap();
            return;
        }
    };

    let track_name = match player.current_track {
        Some(track_name) => track_name,
        None => {
            let edit = EditMessage::new().content(
                "Failed to obtain selected track, please select a track first using /play",
            );
            message.edit(&ctx.http, edit).await.unwrap();
            return;
        }
    };

    let _record = handler
        .gsheet
        .records()
        .create_record(
            msg.id.get(),
            message.id.get(),
            msg.timestamp,
            msg.author.id.get(),
            track_name.clone(),
            time,
        )
        .await;

    let icon_url = handler
        .gsheet
        .tracks()
        .get_all()
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|t| t.name == track_name)
        .map(|t| t.icon_url)
        .unwrap_or_else(|| "https://mario.wiki.gallery/images/thumb/4/47/MKWorldFreeroamWarioWaluigi.png/1600px-MKWorldFreeroamWarioWaluigi.png".to_string());

    let mention = format!("<@{}>", msg.author.id.get());

    let embed = CreateEmbed::default()
        .title("NEW RECORD ADDED")
        .color(Colour::new(0x00b0f4))
        .field("Map", track_name, true)
        .field("Time", duration_to_string(time), true)
        .field("Player", mention, true)
        .image(icon_url);

    let change_driver_button = CreateButton::new("change_driver").label("Change driver");

    let components = vec![CreateActionRow::Buttons(vec![change_driver_button])];

    let edit = EditMessage::new()
        .content("")
        .embed(embed)
        .components(components);

    message.edit(&ctx.http, edit).await.unwrap();
}

pub fn duration_to_string(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    let seconds = duration.as_secs() - minutes * 60;
    let millis = duration.subsec_millis();
    format!("{minutes}:{seconds:0>2}.{millis:0>3}")
}

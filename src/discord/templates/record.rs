use std::time::Duration;

use serenity::all::{Colour, CreateActionRow, CreateButton, CreateEmbed};

use crate::{discord::handler::Handler, sheets::records::record::Record};

pub async fn record_embed(
    record: Record<'_>,
    handler: &Handler,
) -> (CreateEmbed, Vec<CreateActionRow>) {
    let mention = format!("<@{}>", record.driver_user_id);

    let icon_url = handler
        .gsheet
        .tracks()
        .get_all()
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|t| t.name == record.track_name)
        .map(|t| t.icon_url)
        .unwrap_or_else(|| {
            "https://mario.wiki.gallery/images/thumb/4/47/MKWorldFreeroamWarioWaluigi.png/1600px-MKWorldFreeroamWarioWaluigi.png".into()
        });

    let embed = CreateEmbed::default()
        .title("NEW RECORD ADDED")
        .color(Colour::new(0x00b0f4))
        .field("Track", record.track_name, true)
        .field("Time", duration_to_string(record.race_duration), true)
        .field("Player", mention, true)
        .image(icon_url);

    // let change_track_button = change_track_button();
    // let change_time_button = change_time_button();
    let change_driver_button = change_driver_button();

    let components = vec![
        // CreateActionRow::Buttons(vec![change_track_button]),
        // CreateActionRow::Buttons(vec![change_time_button]),
        CreateActionRow::Buttons(vec![change_driver_button]),
    ];

    (embed, components)
}

pub fn duration_to_string(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    let seconds = duration.as_secs() - minutes * 60;
    let millis = duration.subsec_millis();
    format!("{minutes}:{seconds:0>2}.{millis:0>3}")
}

pub fn change_track_button() -> CreateButton {
    CreateButton::new("record_change_track").label("Change track")
}

pub fn change_time_button() -> CreateButton {
    CreateButton::new("record_change_time").label("Change time")
}

pub fn change_driver_button() -> CreateButton {
    CreateButton::new("record_change_driver").label("Change driver")
}
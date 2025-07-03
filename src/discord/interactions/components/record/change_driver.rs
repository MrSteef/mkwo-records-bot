use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
    CreateSelectMenu, CreateSelectMenuKind, UserId,
};

use crate::discord::handler::Handler;

pub async fn handle(ctx: &Context, act: &ComponentInteraction, handler: &Handler) {
    let record_holder = handler
        .gsheet
        .records()
        .get_by_bot_message_id(act.message.id.get())
        .await
        .unwrap()
        .unwrap()
        .driver_user_id;

    let driver_options = CreateSelectMenuKind::User {
        default_users: Some(vec![UserId::new(record_holder)]),
    };

    let driver_dropdown =
        CreateSelectMenu::new("record_select_driver", driver_options).placeholder("No driver selected");

    let message = CreateInteractionResponseMessage::default()
        .ephemeral(true)
        .content("Please select the person that drove this record")
        .select_menu(driver_dropdown);

    let response = CreateInteractionResponse::Message(message);

    act.create_response(&ctx, response).await.unwrap();
}

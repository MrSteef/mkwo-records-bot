use serenity::all::{
    ComponentInteraction, ComponentInteractionDataKind, Context, CreateInteractionResponse,
    EditMessage,
};

use crate::discord::{handler::Handler, templates::record::record_embed};

pub async fn handle(ctx: &Context, act: &ComponentInteraction, handler: &Handler) {
    let bot_message_id = act
        .message
        .clone()
        .message_reference
        .unwrap()
        .message_id
        .unwrap()
        .get();
    let driver_user_id = match &act.data.kind {
        ComponentInteractionDataKind::UserSelect { values } => &values[0],
        _ => panic! {"unexpected interaction data kind"},
    }
    .get();

    let records = handler.gsheet.records();

    let mut record = records
        .get_by_bot_message_id(bot_message_id)
        .await
        .unwrap() // TODO: handle the unwrap properly
        .unwrap(); // TODO: handle the unwrap properly

    record.set_driver_user_id(driver_user_id).await.unwrap(); // TODO: handle the unwrap properly

    let (embed, components) = record_embed(record, handler).await;

    let edit = EditMessage::new()
        .content("")
        .embed(embed)
        .components(components);
    act.channel_id
        .edit_message(&ctx, bot_message_id, edit)
        .await
        .unwrap();
    act.create_response(&ctx, CreateInteractionResponse::Acknowledge)
        .await
        .unwrap();
}

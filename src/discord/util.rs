use serenity::all::{Context, Message};

pub async fn send_chat(ctx: &Context, msg: &Message, text: impl Into<String>) {
    let _ = msg.channel_id.say(&ctx.http, text.into()).await;
}

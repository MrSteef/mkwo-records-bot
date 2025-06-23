use dotenv::dotenv;
use mkwo_records_bot::sheets::GSheet;
use std::env;

use serenity::Client;
use serenity::prelude::*;

use mkwo_records_bot::discord::Handler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let mut gsheet = GSheet::try_new().await.unwrap();

    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in env");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let handler = Handler::try_new(gsheet).await?;

    let mut client = Client::builder(&token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    if let Err(err) = client.start().await {
        eprintln!("Client error: {:?}", err);
    }

    Ok(())
}

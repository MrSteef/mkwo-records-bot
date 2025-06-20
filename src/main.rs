use dotenv::dotenv;
use std::env;

use serenity::prelude::*;
use serenity::Client;

use mkwo_records_bot::discord::Handler;

#[tokio::main]
async fn main() {
    // Load .env vars
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected DISCORD_TOKEN in env");

    let intents =
          GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new())
        .await
        .expect("Error creating client");

    if let Err(err) = client.start().await {
        eprintln!("Client error: {:?}", err);
    }
}
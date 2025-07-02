use std::env;

use dotenv::dotenv;
use mkwo_records_bot::{discord::handler::Handler, sheets::gsheet::GSheet};
use serenity::{all::GatewayIntents, Client};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv()?;
    let gsheet = GSheet::try_new().await?;

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

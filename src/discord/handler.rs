use std::env;

use serenity::{
    all::{AutocompleteChoice, Context, CreateAutocompleteResponse, CreateInteractionResponse, EventHandler, GuildId, Interaction, Message, Ready},
    async_trait,
};

use crate::sheets::GSheet;
use crate::discord::commands::{ocr, play};

pub struct Handler {
    pub gsheet: GSheet,
    pub track_list: Vec<String>,
}

impl Handler {
    pub async fn try_new(gsheet: GSheet) -> anyhow::Result<Self> {
        let track_list = gsheet
            .tracks()
            .get_all()
            .await?
            .into_iter()
            .map(|t| t.name)
            .collect();
        Ok(Handler { gsheet, track_list })
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let guild_id = env::var("GUILD_ID")
            .expect("Expected GUILD_ID")
            .parse::<u64>()
            .expect("GUILD_ID must be u64");
        let guild = GuildId::new(guild_id);

        // Register slash commands
        play::register(&ctx.http, guild).await.unwrap();
        // changetrack::register(&ctx.http, guild).await.unwrap();
        // changedriver::register(&ctx.http, guild).await.unwrap();
    }

    async fn message(&self, ctx: Context, msg: Message) {
        ocr::handle_message(&ctx, &msg).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Autocomplete(ac) => {
                let typed = ac
                    .data
                    .autocomplete()
                    .map_or("", |a| a.value)
                    .to_lowercase();

                let choices: Vec<AutocompleteChoice> = self
                    .track_list
                    .iter()
                    .filter(|n| n.to_lowercase().contains(&typed))
                    .take(25)
                    .map(|n| AutocompleteChoice::new(n, n.clone()))
                    .collect();

                let resp = CreateAutocompleteResponse::new().set_choices(choices);
                let _ = ac
                    .create_response(&ctx.http, CreateInteractionResponse::Autocomplete(resp))
                    .await;
            }
            Interaction::Command(cmd) => {
                match cmd.data.name.as_str() {
                    "play" => play::handle(&ctx, &cmd, self).await,
                    // "changetrack" => changetrack::handle(&ctx, &cmd, self).await,
                    // "changedriver" => changedriver::handle(&ctx, &cmd, self).await,
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

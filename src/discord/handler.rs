use std::env;

use anyhow::Result;
use serenity::{
    all::{Context, EventHandler, GuildId, Interaction, Ready},
    async_trait,
};

use crate::{
    discord::interactions::{autocompletes::track, commands::play},
    sheets::gsheet::GSheet,
};

pub struct Handler {
    pub gsheet: GSheet,
    pub track_name_list: Vec<String>,
}

impl Handler {
    pub async fn try_new(gsheet: GSheet) -> Result<Self> {
        let track_name_list = gsheet
            .tracks()
            .get_all()
            .await?
            .into_iter()
            .map(|t| t.name)
            .collect();
        Ok(Handler {
            gsheet,
            track_name_list,
        })
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let guild_id = env::var("GUILD_ID")
            .expect("Expected GUILD_ID env var")
            .parse::<u64>()
            .expect("GUILD_ID must be u64");
        let guild = GuildId::new(guild_id);

        play::register(&ctx.http, guild).await.unwrap();
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(cmd) => match cmd.data.name.as_str() {
                "play" => play::handle(&ctx, &cmd, &self).await,
                _ => {}
            },
            Interaction::Autocomplete(ac) => match ac
                .data
                .options
                .get(0)
                .and_then(|opt| Some(opt.name.clone()))
                .unwrap_or_default()
                .as_str()
            {
                "track" => track::handle(&ctx, &ac, &self).await,
                _ => {}
            },
            Interaction::Component(act) => match act.data.custom_id.as_str() {
                _ => {}
            },
            _ => {}
        }
    }
}

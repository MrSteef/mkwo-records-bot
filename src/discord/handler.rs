use std::env;

use serenity::{
    all::{
        AutocompleteChoice, Colour, ComponentInteractionDataKind, Context, CreateActionRow,
        CreateAutocompleteResponse, CreateButton, CreateEmbed, CreateInteractionResponse,
        CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuKind, EditMessage,
        EventHandler, GuildId, Interaction, Message, Ready, UserId,
    },
    async_trait,
};

use crate::discord::commands::{
    ocr::{self, duration_to_string},
    play,
};
use crate::sheets::GSheet;

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
        ocr::handle_message(&ctx, &msg, &self).await;
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
            Interaction::Component(act) => {
                // println!("{}", act.data.custom_id);
                match act.data.custom_id.as_str() {
                    "change_driver" => {
                        let record_holder = self
                            .gsheet
                            .records()
                            .get_all()
                            .await
                            .unwrap()
                            .iter()
                            .find(|r| r.bot_message_id == act.message.id.get())
                            .unwrap()
                            .driver_user_id;

                        let driver_options = CreateSelectMenuKind::User {
                            default_users: Some(vec![UserId::new(record_holder)]),
                        };

                        let driver_dropdown =
                            CreateSelectMenu::new("select_driver", driver_options)
                                .placeholder("No driver selected");

                        let message = CreateInteractionResponseMessage::default()
                            .ephemeral(true)
                            .content("Please select the person that drove this record")
                            .select_menu(driver_dropdown);

                        let response = CreateInteractionResponse::Message(message);

                        act.create_response(&ctx, response).await.unwrap();
                    }
                    "select_driver" => {
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

                        self.gsheet
                            .records()
                            .change_driver(bot_message_id, driver_user_id)
                            .await
                            .unwrap();

                        // let edit = EditMessage::new().content("Driver updated!");
                        // let edit = EditInteractionResponse::new().content("Driver changed!");

                        // act.message.edit(&ctx, edit).await.unwrap();

                        // act.edit_followup(&ctx, edit).await.unwrap();

                        // act.message.message_reference.unwrap().message_id.unwrap().get()

                        let record = self
                            .gsheet
                            .records()
                            .get_all()
                            .await
                            .unwrap()
                            .into_iter()
                            .find(|r| r.bot_message_id == bot_message_id)
                            .unwrap();

                        let track_name = record.track_name;

                        let icon_url = self
                            .gsheet
                            .tracks()
                            .get_all()
                            .await
                            .unwrap_or_default()
                            .into_iter()
                            .find(|t| t.name == track_name)
                            .map(|t| t.icon_url)
                            .unwrap_or_else(|| "https://mario.wiki.gallery/images/thumb/4/47/MKWorldFreeroamWarioWaluigi.png/1600px-MKWorldFreeroamWarioWaluigi.png".to_string());

                        let mention = format!("<@{}>", driver_user_id);

                        let embed = CreateEmbed::default()
                            .title("NEW RECORD ADDED")
                            .color(Colour::new(0x00b0f4))
                            .field("Map", track_name, true)
                            .field("Time", duration_to_string(record.race_duration), true)
                            .field("Player", mention, true)
                            .image(icon_url);

                        let change_driver_button =
                            CreateButton::new("change_driver").label("Change driver");

                        let components = vec![CreateActionRow::Buttons(vec![change_driver_button])];

                        let edit = EditMessage::new()
                            .content("")
                            .embed(embed)
                            .components(components);

                        // message.edit(&ctx.http, edit).await.unwrap();

                        act.channel_id
                            .edit_message(&ctx, bot_message_id, edit)
                            .await
                            .unwrap();

                        act.create_response(&ctx, CreateInteractionResponse::Acknowledge)
                            .await
                            .unwrap();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

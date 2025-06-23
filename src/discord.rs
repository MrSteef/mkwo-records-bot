use anyhow::{Result, anyhow};
use std::{env, str::FromStr};

use mime::Mime;
use serenity::{
    all::{
        Attachment, AutocompleteChoice, ChannelId, CommandOptionType, Context,
        CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateInteractionResponse,
        CreateInteractionResponseMessage, EventHandler, GuildId, Interaction, Message, Ready,
    },
    async_trait,
};

use crate::{ocr, sheets::GSheet};

pub struct Handler {
    gsheet: GSheet,
    track_list: Vec<String>,
    // map_selection: HashMap<UserId, String>
}

impl Handler {
    pub async fn try_new(gsheet: GSheet) -> Result<Self> {
        let track_list = gsheet
            .tracks()
            .get_all()
            .await?
            .into_iter()
            .map(|track| track.name)
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

        let play_command_option =
            CreateCommandOption::new(CommandOptionType::String, "track", "Type a track name")
                .set_autocomplete(true)
                .required(true);

        let play_command = CreateCommand::new("play")
            .description("Select a track to play.")
            .add_option(play_command_option);

        let players_command = CreateCommand::new("players")
            .description("Get a list of all players.");

        GuildId::new(guild_id)
            .set_commands(&ctx.http, vec![play_command, players_command])
            .await
            .unwrap();
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let bytes = match validate_all(&msg).await {
            Ok(b) => b,
            Err(err) => {
                match err {
                    ValidationError::Chat(why) => send_chat(&ctx, &msg, why).await,
                    ValidationError::Console(e) => eprintln!("{}", e),
                    ValidationError::Silent(_) => {}
                }
                return;
            }
        };

        match ocr::run_pipeline_from_bytes(&bytes, true, msg.id.get()) {
            Ok(text) => send_chat(&ctx, &msg, format!("Extracted text: {}", text)).await,
            Err(_) => send_chat(&ctx, &msg, "Sorry, I couldn’t process that image.").await,
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Autocomplete(ac) => {
                let typed = ac
                    .data
                    .autocomplete()
                    .map_or_else(|| "", |some| some.value)
                    .to_lowercase();

                let choices: Vec<AutocompleteChoice> = self
                    .track_list
                    .iter()
                    .filter(|name| name.to_lowercase().contains(&typed))
                    .take(25)
                    .map(|name| AutocompleteChoice::new(name, name.clone()))
                    .collect();

                let ac_response = CreateAutocompleteResponse::new().set_choices(choices);

                let response = CreateInteractionResponse::Autocomplete(ac_response);

                if let Err(e) = ac.create_response(&ctx.http, response).await {
                    eprintln!("Failed to respond to autocomplete: {e:?}");
                };
            }
            Interaction::Command(cmd) => match cmd.data.name.as_str() {
                "play" => {
                    let message_response =
                        CreateInteractionResponseMessage::new().content("Track selection received!");

                    let user_id = u64::from(cmd.user.id);

                    let track_name = cmd
                        .data
                        .options
                        .iter()
                        .find(|opt| opt.name == "track")
                        .unwrap()
                        .value
                        .as_str()
                        .unwrap()
                        .to_string();

                    self.gsheet.players().create_player_if_not_exists(user_id).await.unwrap();

                    let selection_result = self
                        .gsheet
                        .players()
                        .select_track(user_id, track_name)
                        .await
                        .unwrap();

                    let response = CreateInteractionResponse::Message(message_response);

                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        eprintln!("{e}");
                    }
                }
                "players" => {
                    let players = self.gsheet.players().get_all().await;

                    let players = match players {
                        Err(why) => {
                            eprint!("{why}");
                            return;
                        }
                        Ok(players) => players,
                    };

                    let message_response =
                        CreateInteractionResponseMessage::new().content(format!("{:?}", players));

                    let response = CreateInteractionResponse::Message(message_response);
                    
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        eprintln!("{e}");
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

enum ValidationError {
    Chat(String),
    Console(String),
    Silent(&'static str),
}

fn validate_channel(msg: &Message) -> Result<(), ValidationError> {
    let channel_id = env::var("CHANNEL_ID")
        .map_err(|_| ValidationError::Console("Failed to get CHANNEL_ID env var".to_string()))?;
    let channel_id = ChannelId::from_str(&channel_id)
        .map_err(|_| ValidationError::Console("Invalid CHANNEL_ID format".to_string()))?;
    if msg.channel_id == channel_id {
        Ok(())
    } else {
        Err(ValidationError::Silent("Wrong channel"))
    }
}

fn validate_from_user(msg: &Message) -> Result<(), ValidationError> {
    if !msg.author.bot {
        Ok(())
    } else {
        Err(ValidationError::Silent("Author is bot"))
    }
}

fn validate_single_attachment(msg: &Message) -> Result<(), ValidationError> {
    if msg.attachments.len() == 1 {
        Ok(())
    } else {
        Err(ValidationError::Silent("Expected one attachment"))
    }
}

fn get_attachment(msg: &Message) -> Result<Attachment, ValidationError> {
    msg.attachments
        .get(0)
        .cloned()
        .ok_or(ValidationError::Chat("No attachment found".to_string()))
}

fn validate_filename_mime_type(att: &Attachment) -> Result<(), ValidationError> {
    let ct = att
        .content_type
        .as_ref()
        .ok_or(ValidationError::Chat("Missing content type".to_string()))?;
    let mime: Mime = ct
        .parse()
        .map_err(|_| ValidationError::Chat("Invalid mime type".to_string()))?;
    if mime.type_() == mime::IMAGE {
        Ok(())
    } else {
        Err(ValidationError::Chat("File is not an image".to_string()))
    }
}

async fn download_attachment(att: Attachment) -> Result<Vec<u8>, ValidationError> {
    att.download()
        .await
        .map_err(|_| ValidationError::Chat("Download failed".to_string()))
}

fn validate_content_mime_type(data: &[u8]) -> Result<(), ValidationError> {
    let info =
        infer::get(data).ok_or(ValidationError::Chat("Cannot infer file type".to_string()))?;
    if info.matcher_type() == infer::MatcherType::Image {
        Ok(())
    } else {
        Err(ValidationError::Chat("Content is not image".to_string()))
    }
}

async fn validate_all(msg: &Message) -> Result<Vec<u8>, ValidationError> {
    validate_channel(msg)?;
    validate_from_user(msg)?;
    validate_single_attachment(msg)?;
    let att = get_attachment(msg)?;
    validate_filename_mime_type(&att)?;
    let data = download_attachment(att).await?;
    validate_content_mime_type(&data)?;
    Ok(data)
}

async fn send_chat(ctx: &Context, msg: &Message, text: impl Into<String>) {
    let _ = msg.channel_id.say(&ctx.http, text.into()).await;
}

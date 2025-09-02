use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateInteractionResponseMessage, EditMessage, GuildId, Http,
};

use crate::{
    discord::{handler::Handler, templates::record::record_embed},
    ocr::parse_duration,
};

#[derive(Debug, thiserror::Error)]
pub enum UpdateTimeCmdError {
    #[error("Command option was missing: {0}")]
    MissingOption(&'static str),

    #[error("Command option was of an incorrect data type: {0}")]
    InvalidOptionType(&'static str),

    #[error("Provided time was not valid: {0}")]
    InvalidTimeFormat(String),

    #[error("Something went wrong while fetching the record")]
    FetchRecord,

    #[error("The record was not found")]
    RecordNotFound,

    #[error("Something went wrong while updating the record time")]
    UpdateFailed,

    #[error("Something went wrong while editing the message")]
    EditFailed,
}

pub async fn handle(ctx: &Context, cmd: &CommandInteraction, handler: &Handler) {
    let outcome = update_time_command(ctx, cmd, handler).await;

    let response_content = match outcome {
        Ok(_) => "Record time updated successfully!".to_string(),
        Err(error) => error.to_string(),
    };

    let _ = cmd
        .create_response(
            &ctx.http,
            serenity::all::CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(response_content)
                    .ephemeral(true),
            ),
        )
        .await;
}

pub async fn update_time_command(
    ctx: &Context,
    cmd: &CommandInteraction,
    handler: &Handler,
) -> Result<(), UpdateTimeCmdError> {
    let bot_message_id = cmd
        .data
        .options
        .iter()
        .find(|opt| opt.name == "message_id")
        // should never be possible if argument is required
        // could consider replacing this with an .expect()
        .ok_or(UpdateTimeCmdError::MissingOption("message_id"))?
        .value
        // should never be anything other than a string
        // could consider replacing this with an .expect()
        .as_str()
        .ok_or(UpdateTimeCmdError::InvalidOptionType("message_id"))?
        .parse::<u64>()
        .map_err(|_| UpdateTimeCmdError::InvalidOptionType("message_id"))?;

    let records = handler.gsheet.records();

    let mut record = records
        .get_by_bot_message_id(bot_message_id)
        .await
        .map_err(|_| UpdateTimeCmdError::FetchRecord)?
        .ok_or(UpdateTimeCmdError::RecordNotFound)?;

    let duration_str = cmd
        .data
        .options
        .iter()
        .find(|opt| opt.name == "record_time")
        // should never be possible if argument is required
        // could consider replacing this with an .expect()
        .ok_or(UpdateTimeCmdError::MissingOption("record_time"))?
        .value
        // should never be anything other than a string
        // could consider replacing this with an .expect()
        .as_str()
        .ok_or(UpdateTimeCmdError::InvalidOptionType("record_time"))?;

    let duration = parse_duration(duration_str)
        .map_err(|e| UpdateTimeCmdError::InvalidTimeFormat(e.to_string()))?;

    record
        .set_race_duration(duration)
        .await
        .map_err(|_| UpdateTimeCmdError::UpdateFailed)?;

    let (embed, components) = record_embed(record, handler).await;

    let edit = EditMessage::new()
        .content("")
        .embed(embed)
        .components(components);

    cmd.channel_id
        .edit_message(&ctx.http, bot_message_id, edit)
        .await
        .map_err(|_| UpdateTimeCmdError::EditFailed)?;

    Ok(())
}

pub async fn register(http: &Http, guild_id: GuildId) -> serenity::Result<()> {
    let update_time_command_option_message = CreateCommandOption::new(
        CommandOptionType::String,
        "message_id",
        "Enter the id of the message of the record that needs to be updated",
    )
    .required(true);
    let update_time_command_option_time = CreateCommandOption::new(
        CommandOptionType::String,
        "record_time",
        "Enter the record time",
    )
    .required(true);

    let update_time_command = CreateCommand::new("update_time")
        .description("Update a record's time")
        .add_option(update_time_command_option_message)
        .add_option(update_time_command_option_time);

    guild_id.create_command(http, update_time_command).await?;

    Ok(())
}

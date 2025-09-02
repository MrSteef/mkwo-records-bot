use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateInteractionResponseMessage, EditMessage, GuildId, Http,
};

use crate::discord::{handler::Handler, templates::record::record_embed};

#[derive(Debug, thiserror::Error)]
pub enum RefreshCmdError {
    #[error("Command option was missing: {0}")]
    MissingOption(&'static str),

    #[error("Command option was of an incorrect data type: {0}")]
    InvalidOptionType(&'static str),

    #[error("Something went wrong while fetching the record")]
    FetchRecord,

    #[error("The record was not found")]
    RecordNotFound,

    #[error("Something went wrong while editing the message")]
    EditFailed,
}

pub async fn handle(ctx: &Context, cmd: &CommandInteraction, handler: &Handler) {
    let outcome = refresh_command(ctx, cmd, handler).await;

    let response_content = match outcome {
        Ok(_) => "Record refreshed successfully!".to_string(),
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

pub async fn refresh_command(
    ctx: &Context,
    cmd: &CommandInteraction,
    handler: &Handler,
) -> Result<(), RefreshCmdError> {
    let bot_message_id = cmd
        .data
        .options
        .iter()
        .find(|opt| opt.name == "message_id")
        // should never be possible if argument is required
        // could consider replacing this with an .expect()
        .ok_or(RefreshCmdError::MissingOption("message_id"))?
        .value
        // should never be anything other than a string
        // could consider replacing this with an .expect()
        .as_str()
        .ok_or(RefreshCmdError::InvalidOptionType("message_id"))?
        .parse::<u64>()
        .map_err(|_| RefreshCmdError::InvalidOptionType("message_id"))?;

    let records = handler.gsheet.records();

    let record = records
        .get_by_bot_message_id(bot_message_id)
        .await
        .map_err(|_| RefreshCmdError::FetchRecord)?
        .ok_or(RefreshCmdError::RecordNotFound)?;

    let (embed, components) = record_embed(record, handler).await;

    let edit = EditMessage::new()
        .content("")
        .embed(embed)
        .components(components);

    cmd.channel_id
        .edit_message(&ctx.http, bot_message_id, edit)
        .await
        .map_err(|_| RefreshCmdError::EditFailed)?;

    Ok(())
}

pub async fn register(http: &Http, guild_id: GuildId) -> serenity::Result<()> {
    let refresh_command_option = CreateCommandOption::new(
        CommandOptionType::String,
        "message_id",
        "Enter the ID of the message you wish to refresh",
    )
    .required(true);

    let refresh_command = CreateCommand::new("refresh")
        .description("Refresh the message of a record.")
        .add_option(refresh_command_option);

    guild_id.create_command(http, refresh_command).await?;

    Ok(())
}

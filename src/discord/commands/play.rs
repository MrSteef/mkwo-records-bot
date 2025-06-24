use serenity::all::{CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, GuildId, Http};

use crate::discord::Handler;

pub async fn register(http: &Http, guild_id: GuildId) -> serenity::Result<()> {
    let play_command_option =
        CreateCommandOption::new(CommandOptionType::String, "track", "Enter a track name")
            .set_autocomplete(true)
            .required(true);

    let play_command = CreateCommand::new("play")
        .description("Select a track to play.")
        .add_option(play_command_option);

    guild_id.create_command(http, play_command).await?;

    Ok(())
}

pub async fn handle(
    ctx: &Context,
    cmd: &CommandInteraction,
    handler: &Handler,
) {
    let response_msg = CreateInteractionResponseMessage::new()
        .content("Track selection received!");

    let user_id = u64::from(cmd.user.id);
    let track_name = cmd
        .data
        .options
        .iter()
        .find(|opt| opt.name == "track")
        .and_then(|opt| opt.value.as_str())
        .unwrap_or_default()
        .to_string();

    if let Err(e) = handler
        .gsheet
        .players()
        .create_player_if_not_exists(user_id)
        .await
    {
        eprintln!("{}", e);
    }
    if let Err(e) = handler
        .gsheet
        .players()
        .select_track(user_id, track_name)
        .await
    {
        eprintln!("{}", e);
    }

    let _ = cmd
        .create_response(&ctx.http, CreateInteractionResponse::Message(response_msg))
        .await;
}

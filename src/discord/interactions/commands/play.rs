use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, GuildId, Http
};

use crate::discord::handler::Handler;

pub enum PlayCmdOutcome {
    Success(String),
    InvalidTrack,
    Failure,
}

pub async fn handle(ctx: &Context, cmd: &CommandInteraction, handler: &Handler) {
    let user_id = u64::from(cmd.user.id);
    let display_name = cmd.user.display_name();
    let track_name = cmd
        .data
        .options
        .iter()
        .find(|opt| opt.name == "track")
        .and_then(|opt| opt.value.as_str())
        .unwrap_or_default()
        .to_string();

    let outcome = play_command(
        user_id,
        display_name.to_string(),
        track_name.clone(),
        handler,
    )
    .await;

    let response = match outcome {
        PlayCmdOutcome::Success(name) => format!("Now playing {}!", name),
        PlayCmdOutcome::InvalidTrack => "Please enter a valid track name".to_string(),
        PlayCmdOutcome::Failure => "Something went wrong, please try again.".to_string(),
    };

    let _ = cmd
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(response),
            ),
        )
        .await;
}

pub async fn play_command(
    user_id: u64,
    display_name: String,
    track_name: String,
    handler: &Handler,
) -> PlayCmdOutcome {
    let is_valid = match handler.gsheet.tracks().get_all().await {
        Ok(tracks) => tracks.iter().any(|t| t.name == track_name),
        Err(_) => return PlayCmdOutcome::Failure,
    };

    if !is_valid {
        return PlayCmdOutcome::InvalidTrack;
    }

    let players = handler.gsheet.players();
    let result = match players.get_by_user_id(user_id).await {
        Err(_) => false,
        Ok(Some(mut player)) => player.set_current_track(track_name.clone()).await.is_ok(),
        Ok(None) => players
            .create(user_id, display_name, Some(track_name.clone()))
            .await
            .is_ok(),
    };

    if result {
        PlayCmdOutcome::Success(track_name)
    } else {
        PlayCmdOutcome::Failure
    }
}

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
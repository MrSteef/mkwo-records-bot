use serenity::all::{AutocompleteChoice, CommandInteraction, Context, CreateAutocompleteResponse, CreateInteractionResponse};

use crate::discord::handler::Handler;

pub async fn handle(ctx: &Context, ac: &CommandInteraction, handler: &Handler) {
    let typed = ac
        .data
        .autocomplete()
        .map_or("", |a| a.value)
        .to_lowercase();

    let choices: Vec<AutocompleteChoice> = handler
        .track_name_list
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

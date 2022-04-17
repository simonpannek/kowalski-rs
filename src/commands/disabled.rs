use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{config::Command, error::KowalskiError, utils::send_response};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    send_response(
        &ctx,
        &command,
        command_config,
        "Disabled command",
        "This bot was built without this feature being enabled.
        Please ask the owner of this bot to rebuilt it with this feature enabled if you wish to use this command.",
    )
    .await
}

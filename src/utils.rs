use serenity::utils::Colour;
use serenity::{
    builder::CreateEmbed,
    client::Context,
    model::interactions::{
        application_command::ApplicationCommandInteraction,
        InteractionResponseType::ChannelMessageWithSource,
    },
    utils::colours::branding::{RED},
};
use tracing::error;

use crate::{config::Command, error::ExecutionError, strings::ERR_CMD_SEND_FAILURE};

/// Send a simple embed response, only giving the title and content.
pub async fn send_response(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    title: &str,
    content: &str,
) -> Result<(), ExecutionError> {
    send_response_complex(ctx, command, command_config, title, content, |embed| embed).await
}

/// Send a complex embed response, giving the title, content and a function further editing the embed.
pub async fn send_response_complex(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    title: &str,
    content: &str,
    f: fn(&mut CreateEmbed) -> &mut CreateEmbed,
) -> Result<(), ExecutionError> {
    let mut embed = create_embed(title, content);
    embed.color(Colour::from((47, 49, 54)));

    // Add module to the footer if the command belongs to a module
    if let Some(module) = &command_config.module {
        embed.footer(|footer| footer.text(format!("Module: {:?}", module)));
    }

    // Apply changed by the given function
    f(&mut embed);

    send_embed(ctx, command, embed).await
}

/// Send a failure embed response, giving the title and content.
pub async fn send_failure(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    title: &str,
    content: &str,
) {
    let mut embed = create_embed(title, content);
    embed.color(RED);

    // If we have failed once already, we only log the error without notifying the user
    if let Err(why) = send_embed(ctx, command, embed).await {
        error!("{}: {}", ERR_CMD_SEND_FAILURE, why);
    }
}

fn create_embed(title: &str, content: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.title(title).description(content);
    embed
}

async fn send_embed(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    embed: CreateEmbed,
) -> Result<(), ExecutionError> {
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(ChannelMessageWithSource)
                .interaction_response_data(|data| data.add_embed(embed))
        })
        .await
        .map_err(|why| ExecutionError::new(&format!("{}", why)))
}

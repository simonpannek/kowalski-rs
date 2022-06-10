use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::User,
    },
};

use crate::{
    config::{Command, Config},
    data,
    error::KowalskiError,
    model::Model,
    utils::{get_relevant_messages, parse_arg_resolved, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config and model
    let (config, model) = data!(ctx, (Config, Model));

    let options = &command.data.options;

    // Parse argument
    let user = if !options.is_empty() {
        match parse_arg_resolved(options, 0)? {
            User(user, ..) => Some(user),
            _ => unreachable!(),
        }
    } else {
        None
    };

    let mut title = "Mood".to_string();

    if let Some(user) = user {
        title.push_str(&format!(" of {}", user.name));
    }

    let messages =
        get_relevant_messages(ctx, &config, command.channel_id, user.map(|user| user.id)).await?;

    if messages.is_empty() {
        return send_response(
            &ctx,
            &command,
            command_config,
            &title,
            "How should I know how someone feels if there are no messages :(",
        )
        .await;
    }

    let result = tokio::task::spawn_blocking(move || {
        let model = model.sentiment.lock().unwrap();

        model.predict(
            &messages
                .iter()
                .map(|message| (message as &dyn AsRef<str>).as_ref())
                .collect::<Vec<_>>(),
        )
    })
    .await
    .unwrap();

    let response = match result.len() {
        1 => {
            format!(
                "The messages are **{:?}**!",
                result.first().unwrap().polarity
            )
        }
        2 => {
            format!(
                "The messages started out **{:?}**. After that they were **{:?}**.",
                result.first().unwrap().polarity,
                result.get(1).unwrap().polarity
            )
        }
        _ => {
            let mut response = format!(
                "The messages started out **{:?}**. The mood then changed to ",
                result.first().unwrap().polarity
            );

            let mut intermediate = Vec::new();

            for i in 1..result.len() - 1 {
                intermediate.push(format!("**{:?}**", result.get(i).unwrap().polarity));
            }

            response.push_str(&intermediate.join(", "));

            response.push_str(&format!(
                ". In the end the messages were **{:?}**.",
                result.last().unwrap().polarity
            ));

            response
        }
    };

    send_response(&ctx, &command, command_config, &title, &response).await
}

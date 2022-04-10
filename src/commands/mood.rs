use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::User,
    },
};

use crate::{
    config::{Command, Config},
    error::ExecutionError,
    model::Model,
    strings::{ERR_API_LOAD, ERR_DATA_ACCESS},
    utils::{get_relevant_messages, parse_arg_resolved, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get config and model
    let (config, model) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let model = data.get::<Model>().expect(ERR_DATA_ACCESS).clone();

        (config, model)
    };

    let options = &command.data.options;

    // Parse argument
    let user = if !options.is_empty() {
        match parse_arg_resolved(options, 0)? {
            User(user, ..) => Ok(Some(user)),
            _ => Err(ExecutionError::new(ERR_API_LOAD)),
        }?
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
        let model = model.sentiment.lock().expect(ERR_DATA_ACCESS);

        model.predict(
            &messages
                .iter()
                .map(|message| (message as &dyn AsRef<str>).as_ref())
                .collect::<Vec<_>>(),
        )
    })
    .await
    .map_err(|why| ExecutionError::new(&format!("{}", why)))?;

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
                "The messages started out **{:?}**. The mood the changed to ",
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

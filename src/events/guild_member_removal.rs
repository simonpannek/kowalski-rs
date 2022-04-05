use serenity::builder::CreateActionRow;
use serenity::model::interactions::application_command::ApplicationCommandInteraction;
use serenity::model::interactions::message_component::ButtonStyle;
use serenity::prelude::Mentionable;
use serenity::{
    client::Context,
    model::{
        guild::Member,
        id::{ChannelId, GuildId},
        user::User,
    },
};
use std::time::Duration;

use crate::{
    config::Config,
    database::client::Database,
    error::ExecutionError,
    strings::ERR_DATA_ACCESS,
    utils::{create_embed, send_response_complex},
};

pub async fn guild_member_removal(
    ctx: &Context,
    guild_id: GuildId,
    user: User,
    _member_data: Option<Member>,
) -> Result<(), ExecutionError> {
    // Get config and database
    let (config, database) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let database = data.get::<Database>().expect(ERR_DATA_ACCESS).clone();

        (config, database)
    };

    // Get possible channels to send scores into
    let channels = {
        let row = database
            .client
            .query_opt(
                "
        SELECT channel FROM score_drops
        WHERE guild = $1::BIGINT
        OFFSET floor(random() * (SELECT COUNT(*) FROM score_drops WHERE guild = $1::BIGINT))
        LIMIT 1
        ",
                &[&i64::from(guild_id)],
            )
            .await?;

        let channel = row.map(|row| ChannelId(row.get::<_, i64>(0) as u64));

        channel
    };

    if let Some(channel) = channels {
        // Get the score of the user
        let score = {
            let row = database
                .client
                .query_one(
                    "
        SELECT SUM(CASE WHEN upvote THEN 1 ELSE -1 END) score
        FROM reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT
        ",
                    &[&i64::from(guild_id), &i64::from(user.id)],
                )
                .await?;

            row.get::<_, Option<i64>>(0).unwrap_or_default()
        };

        let title = format!("User {} has dropped a score of {}", user.name, score);

        // Create action row
        let mut row = CreateActionRow::default();
        row.create_button(|button| {
            button
                .label("Pick up the score")
                .custom_id("pick up")
                .style(ButtonStyle::Primary)
        });

        // Create embed
        let embed = create_embed(
            &title,
            &format!(
                "Click the button to pick up the score of the user {}!",
                user.mention()
            ),
        );

        // Send embed
        let mut message = channel
            .send_message(&ctx.http, |message| {
                message
                    .set_embeds(vec![embed])
                    .components(|components| components.set_action_rows(vec![row]))
            })
            .await?;

        let interaction = message
            .await_component_interaction(&ctx.shard)
            .timeout(Duration::from_secs(config.general.pickup_timeout))
            .await;

        let embed = match interaction {
            Some(interaction) => todo!(),
            None => create_embed(&title, "No one has picked up the reactions in time :("),
        };

        message
            .edit(&ctx.http, |message| {
                message
                    .components(|components| components.set_action_rows(vec![]))
                    .set_embeds(vec![embed])
            })
            .await?;
    }
    Ok(())
}

use serenity::{
    builder::CreateActionRow,
    client::Context,
    model::{
        guild::Member,
        id::{ChannelId, GuildId},
        interactions::message_component::ButtonStyle,
        user::User,
    },
    prelude::Mentionable,
};
use std::time::Duration;

use crate::{
    config::Config, database::client::Database, error::KowalskiError, utils::create_embed,
};

pub async fn guild_member_removal(
    ctx: &Context,
    guild_id: GuildId,
    user: User,
    _member_data: Option<Member>,
) -> Result<(), KowalskiError> {
    // Get config and database
    let (config, database) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().unwrap().clone();
        let database = data.get::<Database>().unwrap().clone();

        (config, database)
    };

    // Select a random channel to send the message to
    let channel = {
        let row = database
            .client
            .query_opt(
                "
        SELECT channel FROM score_drops
        WHERE guild = $1::BIGINT
        OFFSET floor(random() * (SELECT COUNT(*) FROM score_drops WHERE guild = $1::BIGINT))
        LIMIT 1
        ",
                &[&(guild_id.0 as i64)],
            )
            .await?;

        let channel = row.map(|row| ChannelId(row.get::<_, i64>(0) as u64));

        channel
    };

    match channel {
        Some(channel) => {
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
                        &[&(guild_id.0 as i64), &(user.id.0 as i64)],
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
                Some(interaction) => {
                    // Move the reactions to the other user
                    database
                        .client
                        .execute(
                            "
                UPDATE reactions
                SET user_to = $3::BIGINT, native = false
                WHERE guild = $1::BIGINT AND user_to = $2::BIGINT
                ",
                            &[
                                &(guild_id.0 as i64),
                                &(user.id.0 as i64),
                                &(interaction.user.id.0 as i64),
                            ],
                        )
                        .await?;

                    create_embed(
                        &title,
                        &format!(
                            "The user {} has picked up the score of {}!",
                            interaction.user.mention(),
                            user.mention()
                        ),
                    )
                }
                None => {
                    // Delete the reactions on timeout
                    database
                        .client
                        .execute(
                            "
        DELETE FROM reactions
        WHERE guild = $1::BIGINT AND user_to = $2::BIGINT
        ",
                            &[&(guild_id.0 as i64), &(user.id.0 as i64)],
                        )
                        .await?;

                    create_embed(&title, "No one has picked up the reactions in time :(")
                }
            };

            message
                .edit(&ctx.http, |message| {
                    message
                        .components(|components| components.set_action_rows(vec![]))
                        .set_embeds(vec![embed])
                })
                .await?;
        }
        None => {
            // If drops are disabled, just delete the reactions
            database
                .client
                .execute(
                    "
        DELETE FROM reactions
        WHERE guild = $1::BIGINT AND user_to = $2::BIGINT
        ",
                    &[&(guild_id.0 as i64), &(user.id.0 as i64)],
                )
                .await?;
        }
    }

    Ok(())
}

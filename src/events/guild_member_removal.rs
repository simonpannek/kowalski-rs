use std::time::Duration;

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

use crate::{
    config::Config,
    data,
    database::{client::Database, types::ModuleStatus},
    error::KowalskiError,
    utils::create_embed,
};

pub async fn guild_member_removal(
    ctx: &Context,
    guild_id: GuildId,
    user: User,
    _member_data: Option<Member>,
) -> Result<(), KowalskiError> {
    // Get config and database
    let (config, database) = data!(ctx, (Config, Database));

    // Get guild and user ids
    let guild_db_id = guild_id.0 as i64;
    let user_db_id = user.id.0 as i64;

    // Get guild status
    let status = database
        .client
        .query_opt(
            "
                SELECT status
                FROM modules
                WHERE guild = $1::BIGINT
                ",
            &[&guild_db_id],
        )
        .await?
        .map_or(ModuleStatus::default(), |row| row.get(0));

    // Check if the score module is enabled
    if status.score {
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
                    &[&guild_db_id],
                )
                .await?;

            row.map(|row| ChannelId(row.get::<_, i64>(0) as u64))
        };

        if let Some(channel) = channel {
            // Get the score of the user
            let score = {
                let row = database
                    .client
                    .query_one(
                        "
                        SELECT SUM(CASE WHEN upvote THEN 1 ELSE -1 END) score
                        FROM score_reactions r
                        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                        WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT
                        ",
                        &[&guild_db_id, &user_db_id],
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

            match interaction {
                Some(interaction) => {
                    // Get interaction user id
                    let interaction_user_db_id =
                        database.get_user(guild_id, interaction.user.id).await?;

                    // Move the reactions to the other user
                    database
                        .client
                        .execute(
                            "
                            UPDATE score_reactions
                            SET user_to = $3::BIGINT, native = false
                            WHERE guild = $1::BIGINT AND user_to = $2::BIGINT
                            ",
                            &[&guild_db_id, &user_db_id, &interaction_user_db_id],
                        )
                        .await?;

                    let embed = create_embed(
                        &title,
                        &format!(
                            "The user {} has picked up the score of {}!",
                            interaction.user.mention(),
                            user.mention()
                        ),
                    );

                    message
                        .edit(&ctx.http, |message| {
                            message
                                .components(|components| components.set_action_rows(vec![]))
                                .set_embeds(vec![embed])
                        })
                        .await?;

                    return Ok(());
                }
                None => {
                    let embed =
                        create_embed(&title, "No one has picked up the reactions in time :(");

                    message
                        .edit(&ctx.http, |message| {
                            message
                                .components(|components| components.set_action_rows(vec![]))
                                .set_embeds(vec![embed])
                        })
                        .await?;
                }
            };
        }
    }

    // If no drops take place/got picked up, just delete the user
    database
        .client
        .execute(
            "
                DELETE FROM users
                WHERE guild = $1::BIGINT AND \"user\" = $2::BIGINT
                ",
            &[&guild_db_id, &user_db_id],
        )
        .await?;

    Ok(())
}

use std::time::Duration;

use serenity::http::routing::Route;
use serenity::prelude::Mentionable;
use serenity::{
    client::Context,
    model::id::{ChannelId, GuildId, UserId},
};
use tokio::time::interval;

use crate::utils::create_embed;
use crate::{database::client::Database, strings::ERR_DATA_ACCESS};

pub fn check_reminders(ctx: Context, period: Duration) {
    tokio::spawn(async move {
        // Get database
        let database = {
            let data = ctx.data.read().await;

            data.get::<Database>().expect(ERR_DATA_ACCESS).clone()
        };

        // Create the interval at which we will check for reminders
        let mut interval = interval(period);

        loop {
            // Wait for the next tick
            interval.tick().await;

            // Get outstanding reminders
            let reminders = {
                let rows = database
                    .client
                    .query(
                        "
                    DELETE FROM reminders
                    WHERE time <= NOW()
                    RETURNING guild, channel, \"user\", content
                    ",
                        &[],
                    )
                    .await
                    .unwrap();

                rows.iter()
                    .map(|row| {
                        let guild_id = GuildId(row.get::<_, i64>(0) as u64);
                        let channel_id = ChannelId(row.get::<_, i64>(1) as u64);
                        let user_id = UserId(row.get::<_, i64>(2) as u64);
                        let content = row.get::<_, String>(3);

                        (guild_id, channel_id, user_id, content)
                    })
                    .collect::<Vec<_>>()
            };

            for (guild_id, channel_id, user_id, content) in reminders {
                let channel = {
                    let channels = guild_id.channels(&ctx.http).await.unwrap();

                    channels.get(&channel_id).unwrap().clone()
                };

                channel
                    .send_message(&ctx.http, |message| {
                        let embed = create_embed("Reminder", &content);
                        message.content(user_id.mention()).set_embeds(vec![embed])
                    })
                    .await
                    .unwrap();
            }
        }
    });
}

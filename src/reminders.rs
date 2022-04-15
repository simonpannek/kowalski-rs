use std::time::Duration;

use serenity::{
    client::Context,
    model::id::{ChannelId, GuildId, MessageId, UserId},
    prelude::Mentionable,
};
use tokio::time::interval;

use crate::{database::client::Database, strings::ERR_DATA_ACCESS, utils::create_embed};

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
                    RETURNING guild, channel, message, \"user\", content
                    ",
                        &[],
                    )
                    .await
                    .unwrap();

                rows.iter()
                    .map(|row| {
                        let guild_id = GuildId(row.get::<_, i64>(0) as u64);
                        let channel_id = ChannelId(row.get::<_, i64>(1) as u64);
                        let message_id = MessageId(row.get::<_, i64>(2) as u64);
                        let user_id = UserId(row.get::<_, i64>(3) as u64);
                        let content = row.get::<_, String>(4);

                        (guild_id, channel_id, message_id, user_id, content)
                    })
                    .collect::<Vec<_>>()
            };

            for (guild_id, channel_id, message_id, user_id, content) in reminders {
                let channel = {
                    let channels = guild_id.channels(&ctx.http).await.unwrap();

                    channels.get(&channel_id).unwrap().clone()
                };

                let scheduled_message = channel_id.message(&ctx.http, message_id).await;

                channel
                    .send_message(&ctx.http, |message| {
                        if let Ok(scheduled_message) = scheduled_message {
                            message.reference_message((channel_id, scheduled_message.id));
                        }

                        let embed = create_embed("Reminder", &content);
                        message.content(user_id.mention()).set_embeds(vec![embed])
                    })
                    .await
                    .unwrap();
            }
        }
    });
}

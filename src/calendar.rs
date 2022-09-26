use chrono::{Duration, Utc};
use ics::{
    properties::{Description, DtEnd, DtStart, Summary},
    Event, ICalendar,
};
use rocket::{get, routes, State};
use serenity::{client::Context, model::id::GuildId};
use tracing::error;

use crate::{data, database::client::Database, strings::ERR_CALENDAR};

pub fn host_calendar(ctx: Context) {
    tokio::spawn(async move {
        if let Err(why) = rocket::build()
            .manage(ctx)
            .mount("/", routes![events])
            .launch()
            .await
        {
            error!("{}: {}", ERR_CALENDAR, why);
        }
    });
}

#[get("/<id>/events.ics")]
async fn events(ctx: &State<Context>, id: String) -> Option<String> {
    // Get database
    let database = data!(ctx, Database);

    // Get guild id
    let guild_id = {
        let row = database
            .client
            .query_opt(
                "
            SELECT guild FROM publishing
            Where id = $1::TEXT
        ",
                &[&id],
            )
            .await
            .unwrap_or_default();

        row.map(|row| GuildId(row.get::<_, i64>(0) as u64))
    };

    const FORMAT: &str = "%Y%m%dT%H%M%SZ";

    if let Some(guild_id) = guild_id {
        // Get the scheduled events of the guild
        if let Ok(events) = guild_id.scheduled_events(&ctx.http, false).await {
            let mut calendar = ICalendar::new("2.0", "ics-rs");
            for event in events {
                let mut ics_event = Event::new(
                    event.id.0.to_string(),
                    Utc::now().format(FORMAT).to_string(),
                );

                ics_event.push(Summary::new(event.name));
                if let Some(description) = event.description {
                    ics_event.push(Description::new(description));
                }
                ics_event.push(DtStart::new(event.start_time.format(FORMAT).to_string()));
                ics_event.push(DtEnd::new(
                    event
                        .end_time
                        .map(|time| time.naive_utc())
                        .unwrap_or(event.start_time.naive_utc() + Duration::hours(1))
                        .format(FORMAT)
                        .to_string(),
                ));

                calendar.add_event(ics_event);
            }

            return Some(calendar.to_string());
        }
    }

    None
}

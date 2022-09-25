use rocket::{get, routes, State};
use serenity::client::Context;
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

#[get("/<id>")]
async fn events(ctx: &State<Context>, id: String) -> Option<String> {
    // Get database
    let _database = data!(ctx, Database);

    None
}

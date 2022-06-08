use std::{error::Error, fmt::Debug};

use bit_vec::BitVec;
use chrono::{DateTime, Utc};
use serenity::{
    client::Context,
    model::id::{GuildId, RoleId, UserId},
};
use tabled::{builder::Builder, Style, Table};
use tokio_postgres::{
    types::{accepts, private::BytesMut, to_sql_checked, FromSql, IsNull, ToSql, Type},
    Row,
};

/// The status of the modules in a guild. Describes which modules are currently enabled.
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleStatus {
    pub owner: bool,
    pub utility: bool,
    pub score: bool,
    pub reaction_roles: bool,
    pub analyze: bool,
}

/// A table with all fields resolved to a String.
pub struct TableResolved {
    header: Vec<String>,
    rows: Vec<RowResolved>,
}

/// A row with all fields already resolved to a String.
struct RowResolved {
    values: Vec<String>,
}

impl ModuleStatus {
    pub fn default() -> Self {
        ModuleStatus {
            owner: false,
            utility: false,
            score: false,
            reaction_roles: false,
            analyze: false,
        }
    }
}

impl<'a> FromSql<'a> for ModuleStatus {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let bits: BitVec<u32> = FromSql::from_sql(ty, raw)?;

        Ok(ModuleStatus {
            owner: bits.get(0).unwrap_or_default(),
            utility: bits.get(1).unwrap_or_default(),
            score: bits.get(2).unwrap_or_default(),
            reaction_roles: bits.get(3).unwrap_or_default(),
            analyze: bits.get(4).unwrap_or_default(),
        })
    }

    accepts!(BIT);
}

impl ToSql for ModuleStatus {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let mut bits = BitVec::from_elem(8, false);

        bits.set(0, self.owner);
        bits.set(1, self.utility);
        bits.set(2, self.score);
        bits.set(3, self.reaction_roles);
        bits.set(4, self.analyze);

        bits.to_sql(ty, out)
    }

    accepts!(BIT);

    to_sql_checked!();
}

impl TableResolved {
    pub async fn new(ctx: &Context, rows: Vec<Row>) -> Self {
        let header = {
            if rows.is_empty() {
                Vec::new()
            } else {
                rows.get(0)
                    .unwrap()
                    .columns()
                    .iter()
                    .map(|column| column.name().to_string())
                    .collect()
            }
        };
        let mut rows_resolved = Vec::new();

        for row in rows {
            rows_resolved.push(RowResolved::new(ctx, row).await);
        }

        TableResolved {
            header,
            rows: rows_resolved,
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn table(&self, start: usize, length: usize) -> Table {
        // Create iterator for table creation
        let header = vec![self.header.clone()];
        let rows: Vec<_> = self.rows[start..length]
            .iter()
            .map(|row| row.values.clone())
            .collect();

        let iter = header.iter().chain(rows.iter());

        Builder::from_iter(iter).build().with(Style::modern())
    }
}

impl RowResolved {
    pub async fn new(ctx: &Context, row: Row) -> Self {
        let mut values = Vec::new();

        for i in 0..row.len() {
            let column = row.columns().get(i).unwrap();

            match column.type_() {
                &Type::BIT => match column.name() {
                    "status" => {
                        let value: ModuleStatus = row.get(i);
                        values.push(format!("{:?}", value));
                    }
                    _ => {
                        let value: BitVec = row.get(i);
                        values.push(format!("{:?}", value));
                    }
                },
                &Type::BOOL => {
                    let value: bool = row.get(i);

                    values.push(format!("{:?}", value))
                }
                &Type::INT4 => {
                    let value: Option<i32> = row.get(i);

                    values.push(value.map_or("NULL".to_string(), |num| num.to_string()));
                }
                &Type::INT8 => {
                    let value: Option<i64> = row.get(i);

                    match value {
                        Some(value) => {
                            let string =
                                if column.name().starts_with("user") {
                                    // User column
                                    UserId(value as u64)
                                        .to_user(&ctx.http)
                                        .await
                                        .map_or(format!("unknown user ({})", value), |user| {
                                            format!("{}#{:04}", user.name, user.discriminator)
                                        })
                                } else if column.name().starts_with("guild") {
                                    // Guild column
                                    let guild_id = GuildId(value as u64);
                                    match guild_id.to_guild_cached(&ctx.cache) {
                                        Some(guild) => guild.name,
                                        None => guild_id.to_partial_guild(&ctx.http).await.map_or(
                                            format!("unknown guild ({})", value),
                                            |guild| guild.name,
                                        ),
                                    }
                                } else if column.name().starts_with("role") {
                                    // Guild column
                                    let role_id = RoleId(value as u64);
                                    match role_id.to_role_cached(&ctx.cache) {
                                        Some(role) => role.name,
                                        None => role_id
                                            .to_role_cached(&ctx.cache)
                                            .map_or(format!("unknown role ({})", value), |role| {
                                                role.name
                                            }),
                                    }
                                } else {
                                    // Just return the number
                                    value.to_string()
                                };

                            values.push(string)
                        }
                        None => values.push("NULL".to_string()),
                    }
                }
                &Type::TEXT => {
                    let value: Option<String> = row.get(i);

                    match value {
                        Some(value) => values.push(value),
                        None => values.push("NULL".to_string()),
                    }
                }
                &Type::TIMESTAMP | &Type::TIMESTAMPTZ => {
                    let value: DateTime<Utc> = row.get(i);

                    values.push(value.to_rfc2822());
                }
                t => {
                    values.push(format!("unsupported type '{}'", t));
                }
            }
        }

        RowResolved { values }
    }
}

use std::{error::Error, fmt::Debug};

use bit_vec::BitVec;
use serenity::{
    client::Context,
    model::id::{GuildId, UserId},
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
    pub reactions: bool,
    pub reaction_roles: bool,
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
            reactions: false,
            reaction_roles: false,
        }
    }
}

impl<'a> FromSql<'a> for ModuleStatus {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let bits: BitVec<u32> = FromSql::from_sql(ty, raw)?;

        Ok(ModuleStatus {
            owner: bits.get(0).unwrap_or_default(),
            utility: bits.get(1).unwrap_or_default(),
            reactions: bits.get(2).unwrap_or_default(),
            reaction_roles: bits.get(3).unwrap_or_default(),
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
        bits.set(2, self.reactions);
        bits.set(3, self.reaction_roles);

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
                &Type::INT8 => {
                    let value: i64 = row.get(i);

                    let string = if column.name().starts_with("user") {
                        // User column
                        UserId::from(value as u64)
                            .to_user(&ctx.http)
                            .await
                            .map_or(format!("unknown user ({})", value), |user| {
                                format!("{}#{:04}", user.name, user.discriminator)
                            })
                    } else if column.name().starts_with("guild") {
                        // Guild column
                        GuildId::from(value as u64)
                            .to_partial_guild(&ctx.http)
                            .await
                            .map_or(format!("unknown guild ({})", value), |guild| {
                                guild.name.to_string()
                            })
                    } else {
                        // Just return the number
                        value.to_string()
                    };

                    values.push(string)
                }
                &Type::TEXT => {
                    values.push(row.get(i));
                }
                t => {
                    values.push(format!("unsupported type '{}'", t));
                }
            }
        }

        RowResolved { values }
    }
}

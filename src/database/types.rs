use bit_vec::BitVec;
use std::error::Error;
use std::fmt::Debug;
use tokio_postgres::types::private::BytesMut;
use tokio_postgres::types::{accepts, to_sql_checked, FromSql, IsNull, ToSql, Type};

/// The status of the modules in a guild. Describes which modules are currently enabled.
#[derive(Debug)]
pub struct ModuleStatus {
    pub owner: bool,
    pub utility: bool,
    pub reactions: bool,
    pub reaction_roles: bool,
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
        let mut bits = BitVec::with_capacity(8);

        bits.set(0, self.owner);
        bits.set(1, self.utility);
        bits.set(2, self.reactions);
        bits.set(3, self.reaction_roles);

        bits.to_sql(ty, out)
    }

    accepts!(BIT);

    to_sql_checked!();
}

use std::hash::{Hash, Hasher};

use diesel::prelude::*;

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::database::schema::subnetworks)]
#[diesel(primary_key(id))]
pub struct Subnetwork {
    pub id: i16,
    pub subnetwork_id: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::database::schema::subnetworks)]
pub struct SubnetworkInsertable {
    pub subnetwork_id: String,
}

impl Eq for Subnetwork {}

impl PartialEq for Subnetwork {
    fn eq(&self, other: &Self) -> bool {
        self.subnetwork_id == other.subnetwork_id
    }
}

impl Hash for Subnetwork {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.subnetwork_id.hash(state);
    }
}

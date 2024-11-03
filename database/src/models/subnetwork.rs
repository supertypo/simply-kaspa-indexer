#[derive(Clone)]
pub struct Subnetwork {
    pub id: i32,
    pub subnetwork_id: String,
}

impl Eq for Subnetwork {}

impl PartialEq for Subnetwork {
    fn eq(&self, other: &Self) -> bool {
        self.subnetwork_id == other.subnetwork_id
    }
}

impl std::hash::Hash for Subnetwork {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.subnetwork_id.hash(state);
    }
}

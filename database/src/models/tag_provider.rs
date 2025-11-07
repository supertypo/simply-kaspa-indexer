use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub struct TagProvider {
    pub id: i32,
    pub tag: String,
    pub module: Option<String>,
    pub prefix: String,
    pub repository_url: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Eq for TagProvider {}

impl PartialEq for TagProvider {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl std::hash::Hash for TagProvider {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

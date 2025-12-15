use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use sqlx::{Pool, Postgres, Row};
use log::info;

use crate::query::insert::insert_tag_provider;

/// Thread-safe in-memory cache for (tag, module) → tag_id lookups
/// Populated at startup from tag_providers table
#[derive(Clone)]
pub struct TagCache {
    cache: Arc<RwLock<HashMap<(String, String), i32>>>,
}

impl TagCache {
    /// Create a new empty TagCache
    pub fn new() -> Self {
        TagCache {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Build the cache from database by loading all tag_providers
    pub async fn load_from_database(pool: &Pool<Postgres>) -> Result<Self, sqlx::Error> {
        let rows = sqlx::query("SELECT tag, module, id FROM tag_providers")
            .fetch_all(pool)
            .await?;

        let mut map = HashMap::new();
        for row in rows {
            let tag: String = row.try_get(0)?;
            let module: String = row.try_get(1)?;
            let id: i32 = row.try_get(2)?;
            map.insert((tag, module), id);
        }

        info!("TagCache loaded {} tag providers from database", map.len());

        Ok(TagCache {
            cache: Arc::new(RwLock::new(map)),
        })
    }

    /// Upsert a tag provider and add to cache
    /// Returns the tag_id (either existing or newly created)
    pub async fn upsert_tag(
        &self,
        tag: &str,
        module: &str,
        prefix: &str,
        repository: Option<&str>,
        description: Option<&str>,
        category: Option<&str>,
        pool: &Pool<Postgres>,
    ) -> Result<i32, sqlx::Error> {
        // Check cache first (read lock)
        let cache_key = (tag.to_string(), module.to_string());
        {
            let cache = self.cache.read().unwrap();
            if let Some(&id) = cache.get(&cache_key) {
                return Ok(id);
            }
        }

        // Not in cache, insert/update in database
        let tag_id = insert_tag_provider(tag, Some(module), prefix, repository, description, category, pool).await?;

        // Update cache (write lock)
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(cache_key, tag_id);
        }

        Ok(tag_id)
    }

    /// Lookup tag_id by (tag, module) (read-only, fast path)
    pub fn get_tag_id(&self, tag: &str, module: &str) -> Option<i32> {
        let cache = self.cache.read().unwrap();
        cache.get(&(tag.to_string(), module.to_string())).copied()
    }

    /// Refresh cache from database (useful if external changes occur)
    pub async fn refresh(&self, pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
        let rows = sqlx::query("SELECT tag, module, id FROM tag_providers")
            .fetch_all(pool)
            .await?;

        let mut new_map = HashMap::new();
        for row in rows {
            let tag: String = row.try_get(0)?;
            let module: String = row.try_get(1)?;
            let id: i32 = row.try_get(2)?;
            new_map.insert((tag, module), id);
        }

        // Replace cache contents (write lock)
        {
            let mut cache = self.cache.write().unwrap();
            *cache = new_map;
        }

        info!("TagCache refreshed: {} tag providers", self.cache.read().unwrap().len());

        Ok(())
    }

    /// Get the number of cached tags
    pub fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.read().unwrap().is_empty()
    }
}

impl Default for TagCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_cache_new() {
        let cache = TagCache::new();
        assert!(cache.cache.try_read().unwrap().is_empty());
    }

    #[test]
    fn test_tag_cache_operations() {
        let cache = TagCache::new();

        // Initially empty
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());

        // Lookup non-existent tag
        assert_eq!(cache.get_tag_id("nonexistent", "default"), None);

        // Manually insert into cache for testing
        {
            let mut map = cache.cache.write().unwrap();
            map.insert(("test_tag".to_string(), "test_module".to_string()), 42);
        }

        // Lookup existing tag+module
        assert_eq!(cache.get_tag_id("test_tag", "test_module"), Some(42));
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
    }
}

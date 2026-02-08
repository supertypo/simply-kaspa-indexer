use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::mem;
use tokio::sync::RwLock;

pub struct FifoCache<K: Eq + Hash + Clone, V: Clone> {
    map: RwLock<HashMap<K, V>>,
    order: RwLock<VecDeque<K>>,
    capacity: usize,
}

impl<K, V> FifoCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(capacity: usize) -> Self {
        Self { map: RwLock::new(HashMap::with_capacity(capacity)), order: RwLock::new(VecDeque::with_capacity(capacity)), capacity }
    }

    pub async fn get(&self, key: &K) -> Option<V> {
        let map = self.map.read().await;
        map.get(key).cloned()
    }

    pub async fn contains_key(&self, key: &K) -> bool {
        let map = self.map.read().await;
        map.contains_key(key)
    }

    pub async fn with_read_map<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<K, V>) -> R,
    {
        let map = self.map.read().await;
        f(&map)
    }

    pub async fn insert(&self, key: K, value: V) -> Option<V> {
        let mut map = self.map.write().await;
        let mut order = self.order.write().await;

        if map.contains_key(&key) {
            return map.insert(key, value);
        }
        order.push_back(key.clone());
        map.insert(key, value);

        Self::evict_if_needed(&mut map, &mut order, self.capacity);
        None
    }

    pub async fn extend<I>(&self, iter: I)
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut map = self.map.write().await;
        let mut order = self.order.write().await;

        for (key, value) in iter {
            if map.contains_key(&key) {
                map.insert(key, value);
                continue;
            }
            order.push_back(key.clone());
            map.insert(key, value);

            Self::evict_if_needed(&mut map, &mut order, self.capacity);
        }
    }

    pub async fn remove(&self, key: &K) -> Option<V> {
        let mut map = self.map.write().await;
        map.remove(key) // Delay removal from order to keep O(1)
    }

    pub async fn is_empty(&self) -> bool {
        let map = self.map.read().await;
        map.is_empty()
    }

    pub async fn len(&self) -> usize {
        self.map.read().await.len()
    }

    // TODO: Avoid O(len)
    pub async fn last_values(&self, n: usize) -> Vec<V> {
        let map = self.map.read().await;
        let order = self.order.read().await;
        let n = n.min(self.capacity);
        let len = order.len();
        let start = len.saturating_sub(n);
        order.iter().skip(start).filter_map(|k| map.get(k).cloned()).collect()
    }

    pub async fn snapshot(&self) -> Vec<(K, V)> {
        let map = self.map.read().await;
        let order = self.order.read().await;
        order.iter().filter_map(|k| map.get(k).map(|v| (k.clone(), v.clone()))).collect()
    }

    pub async fn drain(&self) -> Vec<(K, V)> {
        let mut map = self.map.write().await;
        let mut order = self.order.write().await;
        let mut drained = Vec::with_capacity(map.len());
        while let Some(k) = order.pop_front() {
            if let Some(v) = map.remove(&k) {
                drained.push((k, v));
            }
        }
        drained
    }

    pub async fn drain_keys_unordered(&self) -> Vec<K> {
        let mut map = self.map.write().await;
        let mut order = self.order.write().await;
        let taken = mem::take(&mut *map);
        order.clear();
        taken.into_keys().collect()
    }

    pub async fn drain_values(&self) -> Vec<V> {
        let mut map = self.map.write().await;
        let mut order = self.order.write().await;
        let mut drained = Vec::with_capacity(map.len());
        while let Some(k) = order.pop_front() {
            if let Some(v) = map.remove(&k) {
                drained.push(v);
            }
        }
        drained
    }

    pub async fn clear(&self) {
        let mut map = self.map.write().await;
        let mut order = self.order.write().await;
        map.clear();
        order.clear();
    }

    fn evict_if_needed(map: &mut HashMap<K, V>, order: &mut VecDeque<K>, capacity: usize) {
        while map.len() > capacity {
            if let Some(old_key) = order.pop_front()
                && map.remove(&old_key).is_some()
            {
                break;
            }
        }
    }
}

impl<K, T> FifoCache<K, Vec<T>>
where
    K: Eq + Hash + Clone,
    T: Clone,
{
    pub async fn flattened_len(&self) -> usize {
        let map = self.map.read().await;
        map.values().map(|v| v.len()).sum()
    }

    pub async fn last_flattened_values(&self, n: usize) -> Vec<T> {
        let map = self.map.read().await;
        let order = self.order.read().await;
        let mut buf: VecDeque<&T> = VecDeque::with_capacity(n);
        for v in order.iter().filter_map(|k| map.get(k)).flatten() {
            if buf.len() == n {
                buf.pop_front();
            }
            buf.push_back(v);
        }
        buf.into_iter().cloned().collect()
    }

    pub async fn drain_flattened_values(&self) -> Vec<T> {
        let mut map = self.map.write().await;
        let mut order = self.order.write().await;
        let mut result = Vec::new();
        while let Some(key) = order.pop_front() {
            if let Some(values) = map.remove(&key) {
                result.extend(values);
            }
        }
        result
    }
}

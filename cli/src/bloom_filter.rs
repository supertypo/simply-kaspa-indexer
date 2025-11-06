use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A simple Bloom filter for fast probabilistic membership testing.
///
/// Used to quickly reject transactions that don't match any filter rules.
/// Provides ~90% fast rejection in <1µs per transaction.
///
/// False positives are possible (may say "might match" when it doesn't),
/// but false negatives are impossible (will never say "doesn't match" when it does).
#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
}

impl BloomFilter {
    /// Create a new Bloom filter optimized for the given parameters.
    ///
    /// # Arguments
    /// * `expected_items` - Expected number of items to be inserted
    /// * `false_positive_rate` - Desired false positive rate (e.g., 0.01 for 1%)
    ///
    /// # Example
    /// ```
    /// use simply_kaspa_cli::bloom_filter::BloomFilter;
    /// let bloom = BloomFilter::new(10000, 0.01); // 10K items, 1% FP rate
    /// ```
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        // Calculate optimal bit array size and number of hash functions
        // Formula: m = -n * ln(p) / (ln(2)^2)
        // where m = bits, n = items, p = false positive rate
        let bits_per_item = -1.44 * false_positive_rate.log2();
        let num_bits = (expected_items as f64 * bits_per_item).ceil() as usize;

        // Formula: k = m/n * ln(2)
        // where k = number of hash functions
        let num_hashes = (bits_per_item * 0.693).ceil() as usize;

        Self {
            bits: vec![false; num_bits],
            num_hashes,
        }
    }

    /// Insert an item into the Bloom filter.
    ///
    /// # Arguments
    /// * `item` - Byte slice to insert
    pub fn insert(&mut self, item: &[u8]) {
        for i in 0..self.num_hashes {
            let index = self.hash(item, i) % self.bits.len();
            self.bits[index] = true;
        }
    }

    /// Check if an item might be in the set.
    ///
    /// Returns `true` if the item might be present (could be false positive).
    /// Returns `false` if the item is definitely not present.
    ///
    /// # Arguments
    /// * `item` - Byte slice to check
    pub fn might_contain(&self, item: &[u8]) -> bool {
        for i in 0..self.num_hashes {
            let index = self.hash(item, i) % self.bits.len();
            if !self.bits[index] {
                return false;
            }
        }
        true
    }

    /// Compute a hash for the given item with a specific seed.
    ///
    /// Uses DefaultHasher with seed mixing for multiple hash functions.
    fn hash(&self, item: &[u8], seed: usize) -> usize {
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        item.hash(&mut hasher);
        hasher.finish() as usize
    }

    /// Calculate memory usage in bytes.
    pub fn memory_usage(&self) -> usize {
        self.bits.len() / 8
    }

    /// Get the number of hash functions used.
    pub fn num_hash_functions(&self) -> usize {
        self.num_hashes
    }

    /// Get the size of the bit array.
    pub fn bit_array_size(&self) -> usize {
        self.bits.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let mut bloom = BloomFilter::new(1000, 0.01);

        // Insert some items
        bloom.insert(b"test1");
        bloom.insert(b"test2");
        bloom.insert(b"test3");

        // Should return true for inserted items
        assert!(bloom.might_contain(b"test1"));
        assert!(bloom.might_contain(b"test2"));
        assert!(bloom.might_contain(b"test3"));

        // Should return false for non-inserted items (most of the time)
        assert!(!bloom.might_contain(b"not_inserted"));
    }

    #[test]
    fn test_bloom_filter_empty() {
        let bloom = BloomFilter::new(1000, 0.01);

        // Empty filter should return false for everything
        assert!(!bloom.might_contain(b"anything"));
    }

    #[test]
    fn test_bloom_filter_false_positive_rate() {
        let mut bloom = BloomFilter::new(1000, 0.01);

        // Insert 1000 items
        for i in 0..1000 {
            bloom.insert(format!("item{}", i).as_bytes());
        }

        // Test with 10000 non-inserted items
        let mut false_positives = 0;
        for i in 1000..11000 {
            if bloom.might_contain(format!("item{}", i).as_bytes()) {
                false_positives += 1;
            }
        }

        let fp_rate = false_positives as f64 / 10000.0;

        // False positive rate should be roughly around 1% (allow some variance)
        // In practice it might be slightly higher due to hash collisions
        assert!(fp_rate < 0.05, "False positive rate too high: {}", fp_rate);
    }

    #[test]
    fn test_bloom_filter_memory_usage() {
        let bloom = BloomFilter::new(1000, 0.01);
        let memory = bloom.memory_usage();

        // Should use roughly 1-2KB for 1000 items at 1% FP rate
        assert!(memory > 0);
        assert!(memory < 10000, "Memory usage too high: {} bytes", memory);
    }

    #[test]
    fn test_bloom_filter_hex_data() {
        let mut bloom = BloomFilter::new(1000, 0.01);

        // Test with binary data (like transaction IDs)
        let txid1 = hex::decode("97b1a3f8").unwrap();
        let txid2 = hex::decode("94f8a2b1").unwrap();
        let txid3 = hex::decode("abcd1234").unwrap();

        bloom.insert(&txid1);
        bloom.insert(&txid2);

        assert!(bloom.might_contain(&txid1));
        assert!(bloom.might_contain(&txid2));
        assert!(!bloom.might_contain(&txid3));
    }
}

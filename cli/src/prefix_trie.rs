use std::collections::HashMap;

/// A prefix trie for efficient O(m) prefix matching where m = prefix length
/// Used for fast transaction filtering with many rules sharing common prefixes
#[derive(Debug, Clone)]
pub struct PrefixTrie {
    root: TrieNode,
}

#[derive(Debug, Clone)]
struct TrieNode {
    children: HashMap<u8, Box<TrieNode>>,
    /// Index into sorted_enabled_rules if this node represents end of a prefix
    rule_index: Option<usize>,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            children: HashMap::new(),
            rule_index: None,
        }
    }
}

impl PrefixTrie {
    /// Create a new empty trie
    pub fn new() -> Self {
        PrefixTrie {
            root: TrieNode::new(),
        }
    }

    /// Insert a prefix into the trie, associating it with a rule index
    ///
    /// # Arguments
    /// * `prefix` - The byte sequence to match (e.g., b"kasplex" or hex bytes)
    /// * `rule_index` - Index into FilterConfig.sorted_enabled_rules
    pub fn insert(&mut self, prefix: &[u8], rule_index: usize) {
        let mut current = &mut self.root;

        for &byte in prefix {
            current = current
                .children
                .entry(byte)
                .or_insert_with(|| Box::new(TrieNode::new()));
        }

        current.rule_index = Some(rule_index);
    }

    /// Lookup the longest matching prefix in the data
    /// Returns the rule_index of the matching rule, or None if no match
    ///
    /// # Arguments
    /// * `data` - The byte sequence to search (e.g., transaction payload)
    ///
    /// # Returns
    /// The rule index if a prefix matches, None otherwise
    pub fn lookup(&self, data: &[u8]) -> Option<usize> {
        let mut current = &self.root;
        let mut last_match: Option<usize> = None;

        for &byte in data {
            // Check if current node represents a valid prefix match
            if let Some(idx) = current.rule_index {
                last_match = Some(idx);
            }

            // Try to continue down the trie
            match current.children.get(&byte) {
                Some(child) => current = child,
                None => break, // No further matches possible
            }
        }

        // Check final node
        if let Some(idx) = current.rule_index {
            last_match = Some(idx);
        }

        last_match
    }

    /// Check if the trie is empty
    pub fn is_empty(&self) -> bool {
        self.root.children.is_empty()
    }

    /// Get the number of nodes in the trie (for diagnostics)
    pub fn node_count(&self) -> usize {
        fn count_nodes(node: &TrieNode) -> usize {
            1 + node.children.values().map(|child| count_nodes(child)).sum::<usize>()
        }
        count_nodes(&self.root)
    }
}

impl Default for PrefixTrie {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert_and_lookup() {
        let mut trie = PrefixTrie::new();
        trie.insert(b"kasplex", 0);
        trie.insert(b"kaspa", 1);

        assert_eq!(trie.lookup(b"kasplex"), Some(0));
        assert_eq!(trie.lookup(b"kaspa-file"), Some(1)); // Prefix match
        assert_eq!(trie.lookup(b"igra"), None);
    }

    #[test]
    fn test_hex_prefixes() {
        let mut trie = PrefixTrie::new();
        trie.insert(&[0x97, 0xb1], 0); // Igra TXID prefix
        trie.insert(&[0x94], 1);       // Igra payload prefix

        assert_eq!(trie.lookup(&[0x97, 0xb1, 0xaf, 0xe0]), Some(0));
        assert_eq!(trie.lookup(&[0x94, 0x02, 0xf8]), Some(1));
        assert_eq!(trie.lookup(&[0x95]), None);
    }

    #[test]
    fn test_overlapping_prefixes() {
        let mut trie = PrefixTrie::new();
        trie.insert(b"kas", 0);
        trie.insert(b"kasplex", 1);
        trie.insert(b"kaspa", 2);

        // Longer prefix should match when available
        assert_eq!(trie.lookup(b"kasplex-protocol"), Some(1));
        assert_eq!(trie.lookup(b"kaspa-file"), Some(2));
        assert_eq!(trie.lookup(b"kaspatalk"), Some(0)); // Falls back to "kas"
    }

    #[test]
    fn test_empty_trie() {
        let trie = PrefixTrie::new();
        assert!(trie.is_empty());
        assert_eq!(trie.lookup(b"anything"), None);
    }

    #[test]
    fn test_node_count() {
        let mut trie = PrefixTrie::new();
        assert_eq!(trie.node_count(), 1); // Just root

        trie.insert(b"abc", 0);
        assert_eq!(trie.node_count(), 4); // root + 3 nodes

        trie.insert(b"abd", 1);
        assert_eq!(trie.node_count(), 5); // Shares "ab", adds "d"
    }

    #[test]
    fn test_single_byte_prefix() {
        let mut trie = PrefixTrie::new();
        trie.insert(b"k", 0);

        assert_eq!(trie.lookup(b"kasplex"), Some(0));
        assert_eq!(trie.lookup(b"k"), Some(0));
        assert_eq!(trie.lookup(b"j"), None);
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub version: String,
    pub settings: FilterSettings,
    pub rules: Vec<FilterRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterSettings {
    pub default_store_payload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterRule {
    pub name: String,
    pub priority: u32,
    pub enabled: bool,
    pub tag: String,
    pub store_payload: bool,
    pub conditions: RuleConditions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConditions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txid: Option<PrefixCondition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Vec<PrefixCondition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixCondition {
    pub prefix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<usize>,
}

impl FilterConfig {
    /// Load and validate filter configuration from a YAML file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let contents = fs::read_to_string(path_ref)
            .map_err(|e| format!("Failed to read config file '{}': {}", path_ref.display(), e))?;

        let config: FilterConfig = serde_yaml::from_str(&contents)
            .map_err(|e| format!("Failed to parse YAML: {}", e))?;

        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration structure and rules
    fn validate(&self) -> Result<(), String> {
        // Version check
        if self.version != "1.0" {
            return Err(format!("Unsupported config version: '{}'. Expected '1.0'", self.version));
        }

        // Ensure at least one rule exists
        if self.rules.is_empty() {
            return Err("Configuration must contain at least one rule".to_string());
        }

        // Rule validation
        for rule in &self.rules {
            self.validate_rule(rule)?;
        }

        // Check for duplicate tags
        let mut seen_tags = HashSet::new();
        for rule in &self.rules {
            if !seen_tags.insert(&rule.tag) {
                return Err(format!("Duplicate tag '{}' found in rules", rule.tag));
            }
        }

        // Check for duplicate rule names
        let mut seen_names = HashSet::new();
        for rule in &self.rules {
            if !seen_names.insert(&rule.name) {
                return Err(format!("Duplicate rule name '{}' found", rule.name));
            }
        }

        Ok(())
    }

    /// Validate a single rule
    fn validate_rule(&self, rule: &FilterRule) -> Result<(), String> {
        // Rule name validation
        if rule.name.is_empty() {
            return Err("Rule name cannot be empty".to_string());
        }

        // Tag validation
        if rule.tag.is_empty() {
            return Err(format!("Rule '{}': tag cannot be empty", rule.name));
        }
        if rule.tag.len() > 50 {
            return Err(format!("Rule '{}': tag must be 1-50 characters (got {})", rule.name, rule.tag.len()));
        }

        // At least one condition must be present
        if rule.conditions.txid.is_none() && rule.conditions.payload.is_none() {
            return Err(format!("Rule '{}': must have at least one condition (txid or payload)", rule.name));
        }

        // Validate txid condition if present
        if let Some(ref txid) = rule.conditions.txid {
            self.validate_prefix_condition(txid, &rule.name, "txid")?;
        }

        // Validate payload conditions if present
        if let Some(ref payload_conditions) = rule.conditions.payload {
            if payload_conditions.is_empty() {
                return Err(format!("Rule '{}': payload conditions array cannot be empty", rule.name));
            }
            for condition in payload_conditions {
                self.validate_prefix_condition(condition, &rule.name, "payload")?;
            }
        }

        Ok(())
    }

    /// Validate a prefix condition
    fn validate_prefix_condition(&self, condition: &PrefixCondition, rule_name: &str, field: &str) -> Result<(), String> {
        if condition.prefix.is_empty() {
            return Err(format!("Rule '{}': {} prefix cannot be empty", rule_name, field));
        }

        // Validate hex prefix format
        if let Some(hex_part) = condition.prefix.strip_prefix("hex:") {
            if hex_part.is_empty() {
                return Err(format!("Rule '{}': {} hex prefix cannot be empty after 'hex:'", rule_name, field));
            }
            // Validate hex characters
            if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!("Rule '{}': {} invalid hex characters in '{}'", rule_name, field, condition.prefix));
            }
            if hex_part.len() % 2 != 0 {
                return Err(format!("Rule '{}': {} hex string must have even length: '{}'", rule_name, field, condition.prefix));
            }
        }

        // Validate length if specified
        if let Some(length) = condition.length {
            if length == 0 {
                return Err(format!("Rule '{}': {} length must be > 0", rule_name, field));
            }
        }

        Ok(())
    }

    /// Get rules sorted by priority (highest first), filtering out disabled rules
    pub fn get_sorted_rules(&self) -> Vec<&FilterRule> {
        let mut rules: Vec<&FilterRule> = self.rules.iter()
            .filter(|r| r.enabled)
            .collect();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority)); // Descending order
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_empty_tag() {
        let config = FilterConfig {
            version: "1.0".to_string(),
            settings: FilterSettings { default_store_payload: false },
            rules: vec![FilterRule {
                name: "test".to_string(),
                priority: 100,
                enabled: true,
                tag: "".to_string(),
                store_payload: true,
                conditions: RuleConditions {
                    txid: None,
                    payload: Some(vec![PrefixCondition {
                        prefix: "test".to_string(),
                        length: None,
                    }]),
                },
            }],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_no_conditions() {
        let config = FilterConfig {
            version: "1.0".to_string(),
            settings: FilterSettings { default_store_payload: false },
            rules: vec![FilterRule {
                name: "test".to_string(),
                priority: 100,
                enabled: true,
                tag: "test".to_string(),
                store_payload: true,
                conditions: RuleConditions {
                    txid: None,
                    payload: None,
                },
            }],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_duplicate_tags() {
        let config = FilterConfig {
            version: "1.0".to_string(),
            settings: FilterSettings { default_store_payload: false },
            rules: vec![
                FilterRule {
                    name: "test1".to_string(),
                    priority: 100,
                    enabled: true,
                    tag: "duplicate".to_string(),
                    store_payload: true,
                    conditions: RuleConditions {
                        txid: None,
                        payload: Some(vec![PrefixCondition {
                            prefix: "test".to_string(),
                            length: None,
                        }]),
                    },
                },
                FilterRule {
                    name: "test2".to_string(),
                    priority: 90,
                    enabled: true,
                    tag: "duplicate".to_string(),
                    store_payload: true,
                    conditions: RuleConditions {
                        txid: None,
                        payload: Some(vec![PrefixCondition {
                            prefix: "test2".to_string(),
                            length: None,
                        }]),
                    },
                },
            ],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_priority_sorting() {
        let config = FilterConfig {
            version: "1.0".to_string(),
            settings: FilterSettings { default_store_payload: false },
            rules: vec![
                FilterRule {
                    name: "low".to_string(),
                    priority: 50,
                    enabled: true,
                    tag: "low".to_string(),
                    store_payload: true,
                    conditions: RuleConditions {
                        txid: None,
                        payload: Some(vec![PrefixCondition {
                            prefix: "low".to_string(),
                            length: None,
                        }]),
                    },
                },
                FilterRule {
                    name: "high".to_string(),
                    priority: 100,
                    enabled: true,
                    tag: "high".to_string(),
                    store_payload: true,
                    conditions: RuleConditions {
                        txid: None,
                        payload: Some(vec![PrefixCondition {
                            prefix: "high".to_string(),
                            length: None,
                        }]),
                    },
                },
            ],
        };

        let sorted = config.get_sorted_rules();
        assert_eq!(sorted[0].priority, 100);
        assert_eq!(sorted[1].priority, 50);
    }
}

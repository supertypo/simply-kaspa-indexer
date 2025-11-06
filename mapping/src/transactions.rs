use log::{debug, trace};
use bigdecimal::ToPrimitive;
use hex;
use kaspa_rpc_core::RpcTransaction;
use simply_kaspa_cli::filter_config::{FilterConfig, FilterRule, PrefixCondition};
use simply_kaspa_database::tag_cache::TagCache;

use simply_kaspa_database::models::address_transaction::AddressTransaction as SqlAddressTransaction;
use simply_kaspa_database::models::block_transaction::BlockTransaction as SqlBlockTransaction;
use simply_kaspa_database::models::script_transaction::ScriptTransaction as SqlScriptTransaction;
use simply_kaspa_database::models::transaction::Transaction as SqlTransaction;
use simply_kaspa_database::models::transaction_input::TransactionInput as SqlTransactionInput;
use simply_kaspa_database::models::transaction_output::TransactionOutput as SqlTransactionOutput;

/// Check if data matches a prefix condition (uses pre-decoded prefix)
fn matches_prefix(data: &[u8], condition: &PrefixCondition) -> bool {
    let decoded_prefix = &condition.decoded_prefix;
    let check_length = condition.length.unwrap_or(decoded_prefix.len()).min(decoded_prefix.len());

    trace!("Checking prefix: '{}', data len: {}, prefix len: {}, check len: {}",
           condition.prefix, data.len(), decoded_prefix.len(), check_length);

    data.len() >= check_length && data[..check_length].starts_with(&decoded_prefix[..check_length])
}

/// Evaluate if a transaction matches a rule (uses pre-decoded prefixes)
fn evaluate_rule(txid_hex: &str, payload: &[u8], rule: &FilterRule) -> bool {
    // Check TXID condition if present
    if let Some(ref txid_condition) = rule.conditions.txid {
        // Use pre-decoded prefix (computed at config load time)
        let decoded_prefix = &txid_condition.decoded_prefix;
        let prefix_str = if txid_condition.prefix.starts_with("hex:") {
            hex::encode(decoded_prefix)
        } else {
            // UTF-8 prefix was stored as bytes, convert back to string
            String::from_utf8_lossy(decoded_prefix).to_string()
        };

        let check_length = txid_condition.length.unwrap_or(prefix_str.len()).min(prefix_str.len());

        if !(txid_hex.starts_with(&prefix_str[..check_length]) && txid_hex.len() >= check_length) {
            trace!("Rule '{}': TXID mismatch", rule.name);
            return false;
        }
        trace!("Rule '{}': TXID match", rule.name);
    }

    // Check payload conditions if present
    if let Some(ref payload_conditions) = rule.conditions.payload {
        let mut payload_matched = false;
        for condition in payload_conditions {
            if matches_prefix(payload, condition) {
                trace!("Rule '{}': Payload matched prefix '{}'", rule.name, condition.prefix);
                payload_matched = true;
                break;
            }
        }
        if !payload_matched {
            trace!("Rule '{}': No payload match", rule.name);
            return false;
        }
    }

    debug!("Rule '{}' MATCHED (tag: {})", rule.name, rule.tag);
    true
}

pub fn map_transaction(
    subnetwork_key: i32,
    transaction: &RpcTransaction,
    include_subnetwork_id: bool,
    include_hash: bool,
    include_mass: bool,
    include_payload: bool,
    include_block_time: bool,
    filter_config: Option<&FilterConfig>,
    tag_cache: Option<&TagCache>,
) -> SqlTransaction {
    let verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");

    let txid_hex = verbose_data.transaction_id.to_string();
    let payload_bytes = &transaction.payload;

    let mut matched_tag: Option<(String, String)> = None; // (tag, module)
    let mut store_payload = false;

    // Apply filtering rules if config is provided
    if let Some(config) = filter_config {
        // Use pre-sorted rules (computed at config load time)
        for rule in &config.sorted_enabled_rules {
            if evaluate_rule(&txid_hex, payload_bytes, rule) {
                let module = rule.module.as_deref().unwrap_or("default");
                matched_tag = Some((rule.tag.clone(), module.to_string()));
                store_payload = rule.store_payload;
                break; // First match wins (highest priority)
            }
        }

        // If no rule matched, use default
        if matched_tag.is_none() {
            store_payload = config.settings.default_store_payload;
        }
    } else {
        // No config = store all payloads (backward compatible)
        store_payload = true;
    }

    // Convert matched tag+module to tag_id via TagCache
    let tag_id = if let Some((tag, module)) = matched_tag {
        tag_cache.and_then(|cache| cache.get_tag_id(&tag, &module))
    } else {
        None
    };

    SqlTransaction {
        transaction_id: verbose_data.transaction_id.into(),
        subnetwork_id: include_subnetwork_id.then_some(subnetwork_key),
        hash: include_hash.then_some(verbose_data.hash.into()),
        mass: (include_mass && verbose_data.compute_mass != 0).then_some(verbose_data.compute_mass.to_i32().unwrap()),
        payload: (store_payload && include_payload && !transaction.payload.is_empty()).then_some(transaction.payload.to_owned()),
        block_time: include_block_time.then_some(verbose_data.block_time.to_i64().unwrap()),
        tag_id,
    }
}

pub fn map_block_transaction(transaction: &RpcTransaction) -> SqlBlockTransaction {
    let verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    SqlBlockTransaction { block_hash: verbose_data.block_hash.into(), transaction_id: verbose_data.transaction_id.into() }
}

pub fn map_transaction_inputs(
    transaction: &RpcTransaction,
    include_previous_outpoint: bool,
    include_signature_script: bool,
    include_sig_op_count: bool,
    include_block_time: bool,
) -> Vec<SqlTransactionInput> {
    let tx_verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    transaction
        .inputs
        .iter()
        .enumerate()
        .map(|(i, input)| SqlTransactionInput {
            transaction_id: tx_verbose_data.transaction_id.into(),
            index: i.to_i16().unwrap(),
            previous_outpoint_hash: include_previous_outpoint.then_some(input.previous_outpoint.transaction_id.into()),
            previous_outpoint_index: include_previous_outpoint.then_some(input.previous_outpoint.index.to_i16().unwrap()),
            signature_script: include_signature_script.then_some(input.signature_script.clone()),
            sig_op_count: include_sig_op_count.then_some(input.sig_op_count as i16),
            block_time: include_block_time.then_some(tx_verbose_data.block_time.to_i64().unwrap()),
            previous_outpoint_script: None,
            previous_outpoint_amount: None,
        })
        .collect::<Vec<SqlTransactionInput>>()
}

pub fn map_transaction_outputs(
    transaction: &RpcTransaction,
    include_amount: bool,
    include_script_public_key: bool,
    include_script_public_key_address: bool,
    include_block_time: bool,
) -> Vec<SqlTransactionOutput> {
    let tx_verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    transaction
        .outputs
        .iter()
        .enumerate()
        .map(|(i, output)| {
            let verbose_data = output.verbose_data.as_ref().expect("Transaction output verbose_data is missing");
            SqlTransactionOutput {
                transaction_id: tx_verbose_data.transaction_id.into(),
                index: i.to_i16().expect("Tx output index is too large for i16"),
                amount: include_amount.then_some(output.value.to_i64().expect("Tx output amount is too large for i64")),
                script_public_key: include_script_public_key.then_some(output.script_public_key.script().to_vec()),
                script_public_key_address: include_script_public_key_address
                    .then_some(verbose_data.script_public_key_address.payload_to_string()),
                block_time: include_block_time.then_some(tx_verbose_data.block_time.to_i64().unwrap()),
            }
        })
        .collect::<Vec<SqlTransactionOutput>>()
}

pub fn map_transaction_outputs_address(transaction: &RpcTransaction) -> Vec<SqlAddressTransaction> {
    let tx_verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    transaction
        .outputs
        .iter()
        .map(|output| {
            let verbose_data = output.verbose_data.as_ref().expect("Transaction output verbose_data is missing");
            SqlAddressTransaction {
                address: verbose_data.script_public_key_address.payload_to_string(),
                transaction_id: tx_verbose_data.transaction_id.into(),
                block_time: tx_verbose_data.block_time.to_i64().unwrap(),
            }
        })
        .collect::<Vec<SqlAddressTransaction>>()
}

pub fn map_transaction_outputs_script(transaction: &RpcTransaction) -> Vec<SqlScriptTransaction> {
    let tx_verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    transaction
        .outputs
        .iter()
        .map(|output| SqlScriptTransaction {
            script_public_key: output.script_public_key.script().to_vec(),
            transaction_id: tx_verbose_data.transaction_id.into(),
            block_time: tx_verbose_data.block_time.to_i64().unwrap(),
        })
        .collect::<Vec<SqlScriptTransaction>>()
}

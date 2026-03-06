use kaspa_rpc_core::{RpcOptionalTransaction, RpcTransaction};

use simply_kaspa_database::models::address_transaction::AddressTransaction as SqlAddressTransaction;
use simply_kaspa_database::models::block_transaction::BlockTransaction as SqlBlockTransaction;
use simply_kaspa_database::models::script_transaction::ScriptTransaction as SqlScriptTransaction;
use simply_kaspa_database::models::transaction::Transaction as SqlTransaction;
use simply_kaspa_database::models::transaction_input::TransactionInput as SqlTransactionInput;
use simply_kaspa_database::models::transaction_output::TransactionOutput as SqlTransactionOutput;
use simply_kaspa_database::models::types::hash::Hash as SqlHash;

pub fn map_transaction(
    subnetwork_key: i32,
    transaction: &RpcTransaction,
    include_subnetwork_id: bool,
    include_hash: bool,
    include_mass: bool,
    include_payload: bool,
    include_block_time: bool,
    include_in: bool,
    include_in_previous_outpoint: bool,
    include_in_signature_script: bool,
    include_in_sig_op_count: bool,
    include_out: bool,
    include_out_amount: bool,
    include_out_script_public_key: bool,
    include_out_script_public_key_address: bool,
) -> SqlTransaction {
    let verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    SqlTransaction {
        transaction_id: verbose_data.transaction_id.into(),
        subnetwork_id: include_subnetwork_id.then_some(subnetwork_key),
        hash: include_hash.then_some(verbose_data.hash.into()),
        mass: (include_mass && verbose_data.compute_mass != 0).then_some(verbose_data.compute_mass as i32),
        payload: (include_payload && !transaction.payload.is_empty()).then_some(transaction.payload.to_owned()),
        block_time: include_block_time.then_some(verbose_data.block_time as i64),
        inputs: include_in
            .then_some(map_transaction_inputs(
                transaction,
                include_in_previous_outpoint,
                include_in_signature_script,
                include_in_sig_op_count,
            ))
            .flatten(),
        outputs: include_out
            .then_some(map_transaction_outputs(
                transaction,
                include_out_amount,
                include_out_script_public_key,
                include_out_script_public_key_address,
            ))
            .flatten(),
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
) -> Option<Vec<SqlTransactionInput>> {
    (!transaction.inputs.is_empty()).then(|| {
        transaction
            .inputs
            .iter()
            .enumerate()
            .map(|(i, input)| SqlTransactionInput {
                index: i as i16,
                previous_outpoint_hash: include_previous_outpoint.then_some(input.previous_outpoint.transaction_id.into()),
                previous_outpoint_index: include_previous_outpoint.then_some(input.previous_outpoint.index as i16),
                signature_script: include_signature_script.then_some(input.signature_script.clone()),
                sig_op_count: include_sig_op_count.then_some(input.sig_op_count as i16),
                previous_outpoint_script: None,
                previous_outpoint_amount: None,
            })
            .collect::<Vec<SqlTransactionInput>>()
    })
}

pub fn map_transaction_outputs(
    transaction: &RpcTransaction,
    include_amount: bool,
    include_script_public_key: bool,
    include_script_public_key_address: bool,
) -> Option<Vec<SqlTransactionOutput>> {
    (!transaction.outputs.is_empty()).then(|| {
        transaction
            .outputs
            .iter()
            .enumerate()
            .map(|(i, output)| {
                let verbose_data = output.verbose_data.as_ref().expect("Transaction output verbose_data is missing");
                SqlTransactionOutput {
                    index: i as i16,
                    amount: include_amount.then_some(output.value as i64),
                    script_public_key: include_script_public_key.then_some(output.script_public_key.script().to_vec()),
                    script_public_key_address: include_script_public_key_address
                        .then_some(verbose_data.script_public_key_address.payload_to_string()),
                }
            })
            .collect::<Vec<SqlTransactionOutput>>()
    })
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
                block_time: tx_verbose_data.block_time as i64,
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
            block_time: tx_verbose_data.block_time as i64,
        })
        .collect::<Vec<SqlScriptTransaction>>()
}

pub fn map_optional_transaction(
    transaction: &RpcOptionalTransaction,
    subnetwork_key: i32,
    include_subnetwork_id: bool,
    include_hash: bool,
    include_mass: bool,
    include_payload: bool,
    include_block_time: bool,
    include_in: bool,
    include_in_previous_outpoint: bool,
    include_in_signature_script: bool,
    include_in_sig_op_count: bool,
    include_out: bool,
    include_out_amount: bool,
    include_out_script_public_key: bool,
    include_out_script_public_key_address: bool,
) -> SqlTransaction {
    let verbose_data = transaction.verbose_data.as_ref().expect("Optional transaction verbose_data is missing");
    SqlTransaction {
        transaction_id: verbose_data.transaction_id.unwrap().into(),
        subnetwork_id: include_subnetwork_id.then_some(subnetwork_key),
        hash: include_hash.then_some(verbose_data.hash.unwrap().into()),
        mass: (include_mass && verbose_data.compute_mass.unwrap() != 0).then_some(verbose_data.compute_mass.unwrap() as i32),
        payload: (include_payload && !transaction.payload.as_ref().unwrap().is_empty())
            .then_some(transaction.payload.as_ref().unwrap().to_owned()),
        block_time: include_block_time.then_some(verbose_data.block_time.unwrap() as i64),
        inputs: include_in
            .then(|| {
                map_optional_transaction_inputs(
                    transaction,
                    include_in_previous_outpoint,
                    include_in_signature_script,
                    include_in_sig_op_count,
                )
            })
            .flatten(),
        outputs: include_out
            .then(|| {
                map_optional_transaction_outputs(
                    transaction,
                    include_out_amount,
                    include_out_script_public_key,
                    include_out_script_public_key_address,
                )
            })
            .flatten(),
    }
}

fn map_optional_transaction_inputs(
    transaction: &RpcOptionalTransaction,
    include_previous_outpoint: bool,
    include_signature_script: bool,
    include_sig_op_count: bool,
) -> Option<Vec<SqlTransactionInput>> {
    (!transaction.inputs.is_empty()).then(|| {
        transaction
            .inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                let utxo = input.verbose_data.as_ref().unwrap().utxo_entry.as_ref().unwrap();
                let outpoint = input.previous_outpoint.as_ref().unwrap();
                SqlTransactionInput {
                    index: i as i16,
                    previous_outpoint_hash: include_previous_outpoint.then(|| outpoint.transaction_id.unwrap().into()),
                    previous_outpoint_index: include_previous_outpoint.then(|| outpoint.index.unwrap() as i16),
                    signature_script: include_signature_script.then(|| input.signature_script.clone().unwrap()),
                    sig_op_count: include_sig_op_count.then(|| input.sig_op_count.unwrap() as i16),
                    previous_outpoint_script: Some(utxo.script_public_key.as_ref().unwrap().script().to_vec()),
                    previous_outpoint_amount: Some(utxo.amount.unwrap() as i64),
                }
            })
            .collect()
    })
}

fn map_optional_transaction_outputs(
    transaction: &RpcOptionalTransaction,
    include_amount: bool,
    include_script_public_key: bool,
    include_script_public_key_address: bool,
) -> Option<Vec<SqlTransactionOutput>> {
    (!transaction.outputs.is_empty()).then(|| {
        transaction
            .outputs
            .iter()
            .enumerate()
            .map(|(i, output)| SqlTransactionOutput {
                index: i as i16,
                amount: include_amount.then(|| output.value.unwrap() as i64),
                script_public_key: include_script_public_key.then(|| output.script_public_key.as_ref().unwrap().script().to_vec()),
                script_public_key_address: include_script_public_key_address
                    .then(|| output.verbose_data.as_ref().unwrap().script_public_key_address.as_ref().unwrap().payload_to_string()),
            })
            .collect()
    })
}

pub fn map_optional_transaction_inputs_address(transaction: &RpcOptionalTransaction) -> Vec<SqlAddressTransaction> {
    let vd = transaction.verbose_data.as_ref().expect("Verbose data missing (RpcDataVerbosityLevel::High required)");
    let tx_id: SqlHash = vd.transaction_id.expect("transaction_id missing in verbose data").into();
    let block_time = vd.block_time.unwrap_or(0) as i64;
    transaction
        .inputs
        .iter()
        .filter_map(|input| {
            input
                .verbose_data
                .as_ref()
                .and_then(|ivd| ivd.utxo_entry.as_ref())
                .and_then(|entry| entry.verbose_data.as_ref().and_then(|evd| evd.script_public_key_address.as_ref()))
                .map(|addr| SqlAddressTransaction { address: addr.payload_to_string(), transaction_id: tx_id.clone(), block_time })
        })
        .filter(|at| !at.address.is_empty())
        .collect()
}

pub fn map_optional_transaction_inputs_script(transaction: &RpcOptionalTransaction) -> Vec<SqlScriptTransaction> {
    let vd = transaction.verbose_data.as_ref().expect("Verbose data missing (RpcDataVerbosityLevel::High required)");
    let tx_id: SqlHash = vd.transaction_id.expect("transaction_id missing in verbose data").into();
    let block_time = vd.block_time.unwrap_or(0) as i64;
    transaction
        .inputs
        .iter()
        .map(|input| {
            let spk = input.verbose_data.as_ref().unwrap().utxo_entry.as_ref().unwrap().script_public_key.as_ref().unwrap();
            SqlScriptTransaction { script_public_key: spk.script().to_vec(), transaction_id: tx_id.clone(), block_time }
        })
        .filter(|st| !st.script_public_key.is_empty())
        .collect()
}

pub fn map_optional_transaction_outputs_address(transaction: &RpcOptionalTransaction) -> Vec<SqlAddressTransaction> {
    let vd = transaction.verbose_data.as_ref().expect("Verbose data missing (RpcDataVerbosityLevel::High required)");
    let tx_id: SqlHash = vd.transaction_id.expect("transaction_id missing in verbose data").into();
    let block_time = vd.block_time.unwrap_or(0) as i64;
    transaction
        .outputs
        .iter()
        .filter_map(|output| {
            output.verbose_data.as_ref().and_then(|ovd| ovd.script_public_key_address.as_ref()).map(|addr| SqlAddressTransaction {
                address: addr.payload_to_string(),
                transaction_id: tx_id.clone(),
                block_time,
            })
        })
        .filter(|at| !at.address.is_empty())
        .collect()
}

pub fn map_optional_transaction_outputs_script(transaction: &RpcOptionalTransaction) -> Vec<SqlScriptTransaction> {
    let vd = transaction.verbose_data.as_ref().expect("Verbose data missing (RpcDataVerbosityLevel::High required)");
    let tx_id: SqlHash = vd.transaction_id.expect("transaction_id missing in verbose data").into();
    let block_time = vd.block_time.unwrap_or(0) as i64;
    transaction
        .outputs
        .iter()
        .filter_map(|output| {
            output.script_public_key.as_ref().map(|spk| SqlScriptTransaction {
                script_public_key: spk.script().to_vec(),
                transaction_id: tx_id.clone(),
                block_time,
            })
        })
        .filter(|st| !st.script_public_key.is_empty())
        .collect()
}

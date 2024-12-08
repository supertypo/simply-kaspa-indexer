use bigdecimal::ToPrimitive;
use kaspa_rpc_core::RpcTransaction;

use simply_kaspa_database::models::address_transaction::AddressTransaction as SqlAddressTransaction;
use simply_kaspa_database::models::block_transaction::BlockTransaction as SqlBlockTransaction;
use simply_kaspa_database::models::transaction::Transaction as SqlTransaction;
use simply_kaspa_database::models::transaction_input::TransactionInput as SqlTransactionInput;
use simply_kaspa_database::models::transaction_output::TransactionOutput as SqlTransactionOutput;

pub fn map_transaction(map_payload: bool, subnetwork_key: i32, transaction: &RpcTransaction) -> SqlTransaction {
    let verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    SqlTransaction {
        transaction_id: verbose_data.transaction_id.into(),
        subnetwork_id: subnetwork_key,
        hash: Some(verbose_data.hash.into()),
        mass: (verbose_data.compute_mass != 0).then_some(verbose_data.compute_mass.to_i32().expect("Tx compute mass is too large")),
        payload: if map_payload && transaction.payload.len() > 0 { Some(transaction.payload.to_owned()) } else { None },
        block_time: verbose_data.block_time.to_i64().expect("Tx block_time is too large"),
    }
}

pub fn map_block_transaction(transaction: &RpcTransaction) -> SqlBlockTransaction {
    let verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    SqlBlockTransaction { block_hash: verbose_data.block_hash.into(), transaction_id: verbose_data.transaction_id.into() }
}

pub fn map_transaction_inputs(transaction: &RpcTransaction) -> Vec<SqlTransactionInput> {
    let tx_verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    transaction
        .inputs
        .to_owned()
        .into_iter()
        .enumerate()
        .map(|(i, input)| SqlTransactionInput {
            transaction_id: tx_verbose_data.transaction_id.into(),
            index: i.to_i16().expect("Tx input index is too large"),
            previous_outpoint_hash: input.previous_outpoint.transaction_id.into(),
            previous_outpoint_index: input.previous_outpoint.index.to_i16().expect("Tx input previous_outpoint_index is too large"),
            signature_script: Some(input.signature_script.clone()),
            sig_op_count: Some(input.sig_op_count as i16),
        })
        .collect::<Vec<SqlTransactionInput>>()
}

pub fn map_transaction_outputs(transaction: &RpcTransaction) -> Vec<SqlTransactionOutput> {
    let tx_verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    transaction
        .outputs
        .to_owned()
        .into_iter()
        .enumerate()
        .map(|(i, output)| {
            let verbose_data = output.verbose_data.as_ref().expect("Transaction output verbose_data is missing");
            SqlTransactionOutput {
                transaction_id: tx_verbose_data.transaction_id.into(),
                index: i.to_i16().expect("Tx output index is too large for i16"),
                amount: output.value.to_i64().expect("Tx output amount is too large for i64"),
                script_public_key: output.script_public_key.script().to_vec(),
                script_public_key_address: Some(verbose_data.script_public_key_address.payload_to_string()),
            }
        })
        .collect::<Vec<SqlTransactionOutput>>()
}

pub fn map_transaction_outputs_address(transaction: &RpcTransaction) -> Vec<SqlAddressTransaction> {
    let tx_verbose_data = transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing");
    transaction
        .outputs
        .to_owned()
        .into_iter()
        .map(|output| {
            let verbose_data = output.verbose_data.as_ref().expect("Transaction output verbose_data is missing");
            SqlAddressTransaction {
                address: verbose_data.script_public_key_address.payload_to_string(),
                transaction_id: tx_verbose_data.transaction_id.into(),
                block_time: tx_verbose_data.block_time.to_i64().expect("Tx block_time is too large"),
            }
        })
        .collect::<Vec<SqlAddressTransaction>>()
}

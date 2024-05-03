// @generated automatically by Diesel CLI.

diesel::table! {
    addresses_transactions (address, transaction_id) {
        address -> Varchar,
        transaction_id -> Bytea,
        block_time -> Nullable<Int8>,
    }
}

diesel::table! {
    blocks (hash) {
        hash -> Bytea,
        accepted_id_merkle_root -> Nullable<Bytea>,
        difficulty -> Nullable<Float8>,
        merge_set_blues_hashes -> Nullable<Array<Nullable<Bytea>>>,
        merge_set_reds_hashes -> Nullable<Array<Nullable<Bytea>>>,
        selected_parent_hash -> Nullable<Bytea>,
        bits -> Nullable<Int8>,
        blue_score -> Nullable<Int8>,
        blue_work -> Nullable<Bytea>,
        daa_score -> Nullable<Int8>,
        hash_merkle_root -> Nullable<Bytea>,
        nonce -> Nullable<Bytea>,
        parents -> Nullable<Array<Nullable<Bytea>>>,
        pruning_point -> Nullable<Bytea>,
        timestamp -> Nullable<Int8>,
        utxo_commitment -> Nullable<Bytea>,
        version -> Nullable<Int2>,
    }
}

diesel::table! {
    blocks_transactions (block_hash, transaction_id) {
        block_hash -> Bytea,
        transaction_id -> Bytea,
    }
}

diesel::table! {
    chain_blocks (block_hash) {
        block_hash -> Bytea,
    }
}

diesel::table! {
    subnetworks (id) {
        id -> Int2,
        #[max_length = 40]
        subnetwork_id -> Varchar,
    }
}

diesel::table! {
    transactions (transaction_id) {
        transaction_id -> Bytea,
        subnetwork_id -> Nullable<Int2>,
        hash -> Nullable<Bytea>,
        mass -> Nullable<Int4>,
        block_time -> Nullable<Int8>,
    }
}

diesel::table! {
    transactions_acceptances (transaction_id) {
        transaction_id -> Bytea,
        block_hash -> Nullable<Bytea>,
    }
}

diesel::table! {
    transactions_inputs (transaction_id, index) {
        transaction_id -> Bytea,
        index -> Int2,
        previous_outpoint_hash -> Nullable<Bytea>,
        previous_outpoint_index -> Nullable<Int2>,
        signature_script -> Nullable<Bytea>,
        sig_op_count -> Nullable<Int2>,
    }
}

diesel::table! {
    transactions_outputs (transaction_id, index) {
        transaction_id -> Bytea,
        index -> Int2,
        amount -> Nullable<Int8>,
        script_public_key -> Nullable<Bytea>,
        script_public_key_address -> Nullable<Varchar>,
    }
}

diesel::table! {
    vars (key) {
        #[max_length = 255]
        key -> Varchar,
        value -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    addresses_transactions,
    blocks,
    blocks_transactions,
    chain_blocks,
    subnetworks,
    transactions,
    transactions_acceptances,
    transactions_inputs,
    transactions_outputs,
    vars,
);

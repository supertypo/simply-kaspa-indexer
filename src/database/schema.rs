// @generated automatically by Diesel CLI.

diesel::table! {
    blocks (hash) {
        hash -> Bytea,
        accepted_id_merkle_root -> Nullable<Bytea>,
        difficulty -> Nullable<Float8>,
        is_chain_block -> Bool,
        merge_set_blues_hashes -> Nullable<Array<Nullable<Bytea>>>,
        merge_set_reds_hashes -> Nullable<Array<Nullable<Bytea>>>,
        selected_parent_hash -> Nullable<Bytea>,
        bits -> Nullable<Int8>,
        blue_score -> Nullable<Int8>,
        blue_work -> Nullable<Bytea>,
        daa_score -> Nullable<Int8>,
        hash_merkle_root -> Nullable<Bytea>,
        nonce -> Nullable<Numeric>,
        parents -> Nullable<Array<Nullable<Bytea>>>,
        pruning_point -> Nullable<Bytea>,
        timestamp -> Nullable<Int4>,
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
    subnetworks (id) {
        id -> Int4,
        #[max_length = 40]
        subnetwork_id -> Varchar,
    }
}

diesel::table! {
    transactions (transaction_id) {
        transaction_id -> Bytea,
        subnetwork -> Nullable<Int4>,
        hash -> Nullable<Bytea>,
        mass -> Nullable<Int4>,
        block_time -> Nullable<Int4>,
        is_accepted -> Bool,
        accepting_block_hash -> Nullable<Bytea>,
    }
}

diesel::table! {
    transactions_inputs (transaction_id, index) {
        transaction_id -> Bytea,
        index -> Int2,
        previous_outpoint_hash -> Bytea,
        previous_outpoint_index -> Int2,
        signature_script -> Bytea,
        sig_op_count -> Int2,
    }
}

diesel::table! {
    transactions_outputs (transaction_id, index) {
        transaction_id -> Bytea,
        index -> Int2,
        amount -> Int8,
        script_public_key -> Bytea,
        #[max_length = 128]
        script_public_key_address -> Varchar,
        #[max_length = 32]
        script_public_key_type -> Varchar,
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
    blocks,
    blocks_transactions,
    subnetworks,
    transactions,
    transactions_inputs,
    transactions_outputs,
    vars,
);

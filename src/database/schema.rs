// @generated automatically by Diesel CLI.

diesel::table! {
    blocks (hash) {
        hash -> Bytea,
        accepted_id_merkle_root -> Nullable<Bytea>,
        difficulty -> Nullable<Float8>,
        is_chain_block -> Nullable<Bool>,
        merge_set_blues_hashes -> Nullable<Array<Nullable<Bytea>>>,
        merge_set_reds_hashes -> Nullable<Array<Nullable<Bytea>>>,
        selected_parent_hash -> Nullable<Bytea>,
        bits -> Int8,
        blue_score -> Int8,
        blue_work -> Bytea,
        daa_score -> Int8,
        hash_merkle_root -> Bytea,
        nonce -> Numeric,
        parents -> Array<Nullable<Bytea>>,
        pruning_point -> Bytea,
        timestamp -> Int4,
        utxo_commitment -> Bytea,
        version -> Int2,
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
        subnetwork -> Int4,
        hash -> Bytea,
        mass -> Int4,
        block_hash -> Array<Nullable<Bytea>>,
        block_time -> Int4,
        is_accepted -> Bool,
        accepting_block_hash -> Nullable<Bytea>,
    }
}

diesel::table! {
    vars (key) {
        #[max_length = 255]
        key -> Varchar,
        value -> Text,
    }
}

diesel::joinable!(transactions -> subnetworks (subnetwork));

diesel::allow_tables_to_appear_in_same_query!(
    blocks,
    subnetworks,
    transactions,
    vars,
);

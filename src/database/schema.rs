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
    vars (key) {
        #[max_length = 255]
        key -> Varchar,
        value -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    blocks,
    vars,
);

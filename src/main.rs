extern crate diesel;

use std::env;
use std::time::Duration;

use diesel::{ExpressionMethods, insert_into, QueryDsl, r2d2, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::ConnectionManager;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use kaspa_wrpc_client::client::ConnectOptions;

use kaspa_db_filler_ng::database::schema::blocks::dsl::blocks;
use kaspa_db_filler_ng::database::schema::blocks::hash;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_manager = ConnectionManager::<PgConnection>::new(&db_url);
    let db_pool = r2d2::Pool::builder()
        .test_on_check_out(true)
        .connection_timeout(Duration::from_secs(10))
        .build(db_manager)
        .expect("Could not build connection pool");
    let db_con = &mut db_pool.get()
        .expect("Unable to get a new connection from the pool");
    println!("Connection to the database established!");

    if env::var("DEVELOPMENT").unwrap().eq_ignore_ascii_case("true") {
        println!("Applying pending migrations");
        db_con.run_pending_migrations(MIGRATIONS)
            .expect("Unable to apply pending migrations");
    }

    println!("Inserting test data");
    for i in 0..9 {
        insert_into(blocks)
            .values(hash.eq(hex::decode(format!("aba{}", i)).expect("")))
            .on_conflict_do_nothing()
            .execute(db_con)
            .expect("Failed to insert test data");
    }

    println!("Fetching test data");
    let results = blocks
        .select(hash)
        .load::<Vec<u8>>(db_con)
        .expect("Failed to retrieve test data");

    println!("Displaying {} blocks:", results.len());
    for r in results {
        println!("hash: {}", hex::encode(r))
    }

    process_blocks().await.expect("");
}

async fn process_blocks() -> Result<(), ()> {
    let url = env::var("KASPAD_RPC_URL").expect("KASPAD_RPC_URL must be set");
    println!("Connecting to kaspad {}", url);
    let client = KaspaRpcClient::new(WrpcEncoding::Borsh, &url)
        .expect("Unable to connect to kaspad");
    client.connect(ConnectOptions::default()).await
        .expect("Unable to connect to kaspad");

    let server_info = client.get_server_info().await
        .expect("Error when invoking GetServerInfo");
    println!("Connected to kaspad version {}", server_info.server_version);

    let block_dag_info = client.get_block_dag_info().await
        .expect("Error when invoking GetBlockDagInfo");
    println!("BlockDagInfo received: pruning_point={}, first_parent={}",
             block_dag_info.pruning_point_hash, block_dag_info.virtual_parent_hashes[0]);

    println!("Getting blocks with lowhash={}", block_dag_info.virtual_parent_hashes[0]);
    let res = client.get_blocks(Some(block_dag_info.virtual_parent_hashes[0]), true, true).await
        .expect("Error when invoking GetBlocks");
    println!("Got {} blocks", res.blocks.len());

    return Ok(());
}

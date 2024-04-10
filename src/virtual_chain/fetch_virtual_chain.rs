extern crate diesel;

use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use kaspa_wrpc_client::KaspaRpcClient;

pub async fn fetch_virtual_chains(start_block_hash: String,
                                  kaspad_client: KaspaRpcClient,
                                  db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {

//    load_start_point(db_pool).await.unwrap();
    Ok(())
}

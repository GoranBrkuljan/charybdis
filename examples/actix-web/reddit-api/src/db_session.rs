use scylla::client::caching_session::CachingSession;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;

// A single contact point: the driver discovers the other nodes from it. Since driver 1.6 every node
// is expected to listen on the port of the contact point, so the nodes that docker-compose publishes
// on other host ports (9043, 9044) must not be listed here.
const NODE: &str = "127.0.0.1:9042";
const KEYSPACE: &str = "reddit_example";
const CACHE_SIZE: usize = 1000;

/// For test use only; do not reinitialize in production.
pub async fn db_session() -> CachingSession {
    let db_session = default_session().await;

    CachingSession::from(db_session, CACHE_SIZE)
}

pub async fn default_session() -> Session {
    SessionBuilder::new()
        .known_node(NODE)
        .use_keyspace(KEYSPACE, false)
        .build()
        .await
        .expect("Unable to connect to scylla hosts")
}

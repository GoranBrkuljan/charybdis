use scylla::client::caching_session::CachingSession;
use scylla::client::session_builder::SessionBuilder;
use std::time::Duration;

// A single contact point: the driver discovers the other nodes from it. Since driver 1.6 every node
// is expected to listen on the port of the contact point, so the nodes that docker-compose publishes
// on other host ports (9043, 9044) must not be listed here.
const NODE: &str = "127.0.0.1:9042";
const KEYSPACE: &str = "charybdis";
const CACHE_SIZE: usize = 1000;

const EVENTUALLY_ATTEMPTS: u32 = 100;
const EVENTUALLY_INTERVAL: Duration = Duration::from_millis(100);

pub async fn db_session() -> CachingSession {
    let db_session = SessionBuilder::new()
        .known_node(NODE)
        .use_keyspace(KEYSPACE, false)
        .build()
        .await
        .expect("Unable to connect to scylla hosts");
    CachingSession::from(db_session, CACHE_SIZE)
}

/// Secondary indexes and materialized views are updated asynchronously, so a read that goes through
/// them right after a write may not see it yet. Polls `check` until it yields a value.
pub async fn eventually<T, F, Fut>(mut check: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    for _ in 0..EVENTUALLY_ATTEMPTS {
        if let Some(value) = check().await {
            return value;
        }

        tokio::time::sleep(EVENTUALLY_INTERVAL).await;
    }

    panic!(
        "condition not met within {:?}",
        EVENTUALLY_INTERVAL * EVENTUALLY_ATTEMPTS
    );
}

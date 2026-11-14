use crate::common::db_session;
use charybdis::errors::CharybdisError;
use charybdis::macros::charybdis_model;
use charybdis::operations::Delete;
use charybdis::types::{Counter, Uuid};

#[charybdis_model(
    table_name = post_counters,
    partition_keys = [id],
    clustering_keys = [],
    exclude_partial_model = true
)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PostCounter {
    pub id: Uuid,
    pub likes: Counter,
    pub comments: Option<Counter>,
}

#[tokio::test]
async fn increment_and_decrement() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let post_counter = PostCounter {
        id: Uuid::new_v4(),
        ..Default::default()
    };

    post_counter.increment_likes(3).execute(db_session).await?;
    post_counter.decrement_likes(1).execute(db_session).await?;

    // a counter that was never written is null
    let found = PostCounter::find_by_id(post_counter.id).execute(db_session).await?;
    assert_eq!(found.likes, Counter(2));
    assert_eq!(found.comments, None);

    post_counter.increment_comments(10).execute(db_session).await?;
    post_counter.decrement_comments(15).execute(db_session).await?;

    let found = PostCounter::find_by_id(post_counter.id).execute(db_session).await?;
    assert_eq!(found.likes, Counter(2));
    assert_eq!(found.comments, Some(Counter(-5)));

    post_counter.delete().execute(db_session).await?;

    assert_eq!(
        PostCounter::maybe_find_first_by_id(post_counter.id)
            .execute(db_session)
            .await?,
        None
    );

    Ok(())
}

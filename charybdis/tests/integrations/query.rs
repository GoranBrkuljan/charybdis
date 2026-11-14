use crate::common::db_session;
use crate::custom_fields::AddressTypeCustomField;
use crate::model::{Post, User};
use charybdis::batch::ModelBatch;
use charybdis::errors::CharybdisError;
use charybdis::operations::{Delete, Find, Insert, Update};
use charybdis::scylla::PagingStateResponse;
use charybdis::stream::CharybdisModelStream;

#[tokio::test]
async fn model_insert() {
    let id = uuid::Uuid::new_v4();
    let new_user = User::homer(id);

    let db_session = db_session().await;

    new_user
        .insert()
        .execute(&db_session)
        .await
        .expect("Failed to insert user");

    let user = User::find_by_id(id)
        .execute(&db_session)
        .await
        .expect("Failed to find user");

    assert_eq!(user, new_user);

    // `insert_if_not_exists` leaves a row that is already there as it is
    let mut changed_user = new_user.clone();
    changed_user.bio = Some("I like beer".to_string());

    changed_user
        .insert_if_not_exists()
        .execute(&db_session)
        .await
        .expect("Failed to insert user if not exists");

    let user = User::find_by_id(id)
        .execute(&db_session)
        .await
        .expect("Failed to find user");

    assert_eq!(user, new_user);

    user.delete().execute(&db_session).await.expect("Failed to delete user");
}

#[tokio::test]
async fn model_update() {
    let id = uuid::Uuid::new_v4();
    let new_user = User::homer(id);

    let db_session = db_session().await;

    new_user
        .insert()
        .execute(&db_session)
        .await
        .expect("Failed to insert user");

    let mut user = new_user.clone();
    user.bio = Some("I like beer".to_string());
    user.address.as_mut().expect("homer should have address").addr_type = AddressTypeCustomField::HomeAddress;
    let tag = ("Second Key".to_string(), "Second Value".to_string());
    user.user_extra_data.user_tags.push(tag.clone());

    user.update().execute(&db_session).await.expect("Failed to update user");

    let updated_user = User::find_by_id(id)
        .execute(&db_session)
        .await
        .expect("Failed to find user");

    assert_eq!(updated_user.bio, Some("I like beer".to_string()));
    assert_eq!(
        updated_user
            .address
            .as_ref()
            .expect("homer should have address")
            .addr_type,
        AddressTypeCustomField::HomeAddress
    );
    assert_eq!(updated_user.user_extra_data.user_tags.last().cloned(), Some(tag));

    // and the fields that were not changed are still there
    assert_eq!(updated_user, user);

    user.delete().execute(&db_session).await.expect("Failed to delete user");
}

#[tokio::test]
async fn model_delete() {
    let id = uuid::Uuid::new_v4();
    let new_user = User::homer(id);

    let db_session = db_session().await;

    new_user
        .insert()
        .execute(&db_session)
        .await
        .expect("Failed to insert user");

    new_user
        .delete()
        .execute(&db_session)
        .await
        .expect("Failed to delete user");

    let user = User::maybe_find_first_by_id(id)
        .execute(&db_session)
        .await
        .expect("Failed to find user");

    assert_eq!(user, None);
}

#[tokio::test]
async fn model_row() {
    let id = uuid::Uuid::new_v4();
    let new_user = User::homer(id);

    let db_session = db_session().await;

    new_user
        .insert()
        .execute(&db_session)
        .await
        .expect("Failed to insert user");

    let user = User::find_by_id(id)
        .execute(&db_session)
        .await
        .expect("Failed to find user");

    assert_eq!(user, new_user);

    user.delete().execute(&db_session).await.expect("Failed to delete user");
}

#[tokio::test]
async fn optional_model_row() {
    let id = uuid::Uuid::new_v4();
    let db_session = db_session().await;

    let user = User::maybe_find_first_by_id(id)
        .execute(&db_session)
        .await
        .expect("Failed to find user");

    assert_eq!(user, None);
}

#[tokio::test]
async fn model_stream() {
    let db_session = db_session().await;

    let sample_users = User::populate_sample_users().await;

    let users: CharybdisModelStream<User> = User::find_all()
        .execute(&db_session)
        .await
        .expect("Failed to find users");
    let users_vec = users.try_collect().await.expect("Failed to collect users");

    // other tests write to this table while this one runs, so only the users inserted here are checked
    for sample_user in &sample_users {
        assert!(
            users_vec.contains(sample_user),
            "User {} is missing from the stream",
            sample_user.username
        );
    }

    User::delete_batch()
        .chunked_delete(&db_session, &sample_users, 100)
        .await
        .expect("Failed to delete users");
}

#[tokio::test]
async fn model_paged() {
    let db_session = db_session().await;
    let category_id = uuid::Uuid::new_v4();

    Post::populate_sample_posts_per_partition(category_id, None).await;

    let (posts, paging_state_response) = Post::find_by_partition_key_value_paged((category_id,))
        .page_size(3)
        .execute(&db_session)
        .await
        .expect("Failed to find posts");

    let posts = posts
        .collect::<Result<Vec<Post>, CharybdisError>>()
        .expect("Failed to collect posts");

    assert_eq!(posts.len(), 3);
    assert_eq!(posts[0].order_idx, 0);
    assert_eq!(posts[1].order_idx, 1);
    assert_eq!(posts[2].order_idx, 2);
    assert!(
        matches!(paging_state_response, PagingStateResponse::HasMorePages { .. }),
        "Expected more pages, but got NoMorePages"
    );

    if let PagingStateResponse::HasMorePages { state } = paging_state_response {
        let (next_page_posts, paging_state_response) = Post::find_by_partition_key_value_paged((category_id,))
            .page_size(30) // 32 is the total number of posts
            .paging_state(state)
            .execute(&db_session)
            .await
            .expect("Failed to find posts");
        let next_page_posts = next_page_posts
            .collect::<Result<Vec<Post>, CharybdisError>>()
            .expect("Failed to collect posts");

        assert_eq!(next_page_posts.len(), 29);
        assert_eq!(next_page_posts[0].order_idx, 3);
        assert_eq!(next_page_posts[1].order_idx, 4);
        assert_eq!(next_page_posts[28].order_idx, 31);

        assert!(
            matches!(paging_state_response, PagingStateResponse::NoMorePages),
            "Expected no more pages, but got HasMorePages"
        );

        Post::delete_batch()
            .chunked_delete(&db_session, &next_page_posts, 100)
            .await
            .expect("Failed to delete posts");
    }

    Post::delete_batch()
        .chunked_delete(&db_session, &posts, 100)
        .await
        .expect("Failed to delete posts");
}

#[tokio::test]
async fn model_multi_field_paged() {
    let db_session = db_session().await;
    let category_id = uuid::Uuid::new_v4();
    let author_id = uuid::Uuid::new_v4();
    let repeated_order_idx = 42;

    let posts_to_insert = (0..5)
        .map(|i| Post {
            category_id,
            order_idx: repeated_order_idx,
            title: format!("Paged Post {i}"),
            content: format!("Paged content {i}"),
            author_id,
        })
        .collect::<Vec<Post>>();

    Post::batch()
        .chunked_insert(&db_session, &posts_to_insert, 100)
        .await
        .expect("Failed to insert paged posts");

    let (first_page_iter, paging_state_response) =
        Post::find_by_category_id_and_order_idx_paged(category_id, repeated_order_idx)
            .page_size(2)
            .execute(&db_session)
            .await
            .expect("Failed to fetch first page of posts");

    let first_page = first_page_iter
        .collect::<Result<Vec<Post>, CharybdisError>>()
        .expect("Failed to collect first paged posts");

    assert_eq!(first_page.len(), 2);
    assert!(first_page.iter().all(|post| post.order_idx == repeated_order_idx));
    assert!(
        matches!(paging_state_response, PagingStateResponse::HasMorePages { .. }),
        "Expected paged query to have additional pages"
    );

    let mut collected_posts = first_page.clone();

    if let PagingStateResponse::HasMorePages { state } = paging_state_response {
        let (next_page_iter, next_paging_state_response) =
            Post::find_by_category_id_and_order_idx_paged(category_id, repeated_order_idx)
                .page_size(10)
                .paging_state(state)
                .execute(&db_session)
                .await
                .expect("Failed to fetch second page of posts");

        let next_page = next_page_iter
            .collect::<Result<Vec<Post>, CharybdisError>>()
            .expect("Failed to collect second paged posts");

        assert_eq!(next_page.len(), 3);
        assert!(next_page.iter().all(|post| post.order_idx == repeated_order_idx));
        assert!(
            matches!(next_paging_state_response, PagingStateResponse::NoMorePages),
            "Expected paged query to finish after the second call"
        );

        collected_posts.extend(next_page);
    } else {
        panic!("Expected additional pages for multi-field paged query");
    }

    assert_eq!(collected_posts.len(), posts_to_insert.len());

    Post::delete_batch()
        .chunked_delete(&db_session, &posts_to_insert, 100)
        .await
        .expect("Failed to delete paged posts");
}

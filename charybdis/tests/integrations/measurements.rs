use crate::common::db_session;
use charybdis::batch::ModelBatch;
use charybdis::errors::CharybdisError;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, Update};
use charybdis::types::{Blob, Boolean, Int, Text, Timestamp, Uuid};

/// A primary key of five columns: finders are generated for every prefix of the key that contains the
/// whole partition key, while `delete_by_...` helpers are generated for the first three columns only.
#[charybdis_model(
    table_name = measurements,
    partition_keys = [device_id],
    clustering_keys = [channel, metric, taken_at, is_estimate],
    global_secondary_indexes = [],
    local_secondary_indexes = []
)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Measurement {
    pub device_id: Uuid,
    pub channel: Int,
    pub metric: Text,
    pub taken_at: Timestamp,
    // an optional key column is passed to the generated functions without the `Option`
    pub is_estimate: Option<Boolean>,
    pub payload: Option<Blob>,
}

partial_measurement!(
    UpdatePayloadMeasurement,
    device_id,
    channel,
    metric,
    taken_at,
    is_estimate,
    payload
);

#[tokio::test]
async fn finders_and_deletes_by_key_prefix() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let device_id = Uuid::new_v4();
    let taken_at = chrono::DateTime::from_timestamp_millis(1_700_000_000_123).expect("valid timestamp");

    // two channels with three metrics each, already in clustering order
    let measurements = (0..6_u8)
        .map(|i| Measurement {
            device_id,
            channel: Int::from(i / 3),
            metric: format!("metric_{}", i % 3),
            taken_at,
            is_estimate: Some(i % 2 == 0),
            payload: Some(vec![i]),
        })
        .collect::<Vec<Measurement>>();

    Measurement::batch()
        .chunked_insert(db_session, &measurements, 4)
        .await?;

    let by_device = Measurement::find_by_device_id(device_id)
        .execute(db_session)
        .await?
        .try_collect()
        .await?;
    assert_eq!(by_device, measurements);

    let by_channel = Measurement::find_by_device_id_and_channel(device_id, 0)
        .execute(db_session)
        .await?
        .try_collect()
        .await?;
    assert_eq!(by_channel, measurements[0..3].to_vec());

    let first_of_channel = Measurement::find_first_by_device_id_and_channel(device_id, 1)
        .execute(db_session)
        .await?;
    assert_eq!(first_of_channel, measurements[3]);

    let by_primary_key = Measurement::find_by_device_id_and_channel_and_metric_and_taken_at_and_is_estimate(
        device_id,
        1,
        "metric_1".to_string(),
        taken_at,
        true,
    )
    .execute(db_session)
    .await?;
    assert_eq!(by_primary_key, measurements[4]);

    // a partial model has the same finders, also for a key this long
    let mut partial = UpdatePayloadMeasurement::find_by_device_id_and_channel_and_metric_and_taken_at_and_is_estimate(
        device_id,
        1,
        "metric_1".to_string(),
        taken_at,
        true,
    )
    .execute(db_session)
    .await?;
    partial.payload = None;
    partial.update().execute(db_session).await?;

    assert_eq!(
        by_primary_key.find_by_primary_key().execute(db_session).await?.payload,
        None
    );

    Measurement::delete_by_device_id_and_channel_and_metric(device_id, 0, "metric_0".to_string())
        .execute(db_session)
        .await?;
    Measurement::delete_by_device_id_and_channel(device_id, 1)
        .execute(db_session)
        .await?;

    let remaining = Measurement::find_by_device_id(device_id)
        .execute(db_session)
        .await?
        .try_collect()
        .await?;
    assert_eq!(remaining, measurements[1..3].to_vec());

    Measurement::delete_by_device_id(device_id).execute(db_session).await?;

    let remaining = Measurement::find_by_device_id(device_id)
        .execute(db_session)
        .await?
        .try_collect()
        .await?;
    assert!(remaining.is_empty());

    Ok(())
}

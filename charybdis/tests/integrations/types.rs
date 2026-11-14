use crate::common::{db_session, eventually};
use crate::custom_fields::UserExtraDataCustomField;
use crate::measurements::Measurement;
use charybdis::batch::{CharybdisBatch, ModelBatch};
use charybdis::errors::CharybdisError;
use charybdis::macros::{charybdis_model, charybdis_udt_model, charybdis_view_model};
use charybdis::model::{AsNative, BaseModel};
use charybdis::operations::{Delete, Find, Insert, Update};
use charybdis::serializers::{FromJson, ToJson};
use charybdis::types::{
    Ascii, BigInt, Blob, Boolean, Date, Decimal, Double, Duration, Float, Frozen, Inet, Int, List, Map, Set, SmallInt,
    Text, Time, Timestamp, Timeuuid, TinyInt, Tuple, Uuid, Varchar, Varint,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

const NODE_ID: [u8; 6] = [1, 2, 3, 4, 5, 6];

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[charybdis_udt_model(type_name = label)]
pub struct Label {
    pub name: Text,
    pub weight: Option<Int>,
    pub aliases: Frozen<List<Text>>,
}

/// One column of every supported type, plus the model options that the other test models do not use:
/// a composite partition key, a static column, table options and an ignored field.
#[charybdis_model(
    table_name = types,
    partition_keys = [tenant_id, bucket],
    clustering_keys = [created_at, id],
    static_columns = [tenant_name],
    global_secondary_indexes = [email],
    local_secondary_indexes = [status],
    table_options = r#"
        CLUSTERING ORDER BY (created_at DESC, id ASC)
        AND gc_grace_seconds = 86400
    "#
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Types {
    pub tenant_id: Uuid,
    pub bucket: Int,
    pub created_at: Timestamp,
    pub id: Timeuuid,
    pub tenant_name: Option<Text>,
    #[serde(rename = "mail")]
    pub email: Text,
    pub status: Option<Text>,
    pub ascii: Ascii,
    pub big_int: BigInt,
    pub blob: Blob,
    pub boolean: Boolean,
    pub date: Date,
    pub decimal: Decimal,
    pub double: Double,
    pub duration: Option<Duration>,
    pub float: Float,
    pub inet: Option<Inet>,
    pub small_int: SmallInt,
    pub time: Time,
    pub tiny_int: TinyInt,
    pub varchar: Varchar,
    pub varint: Varint,
    pub tags: Set<Text>,
    pub items: List<Int>,
    pub attrs: Map<Text, Text>,
    pub frozen_items: Frozen<List<Text>>,
    pub pair: Frozen<Tuple<Int, Text>>,
    pub nested: Option<Map<Text, Frozen<List<Uuid>>>>,
    pub label: Option<Label>,
    pub labels: Option<List<Frozen<Label>>>,
    #[charybdis(column_type = "Text")]
    #[serde(default)]
    pub custom: UserExtraDataCustomField,
    #[charybdis(ignore)]
    #[serde(skip)]
    pub transient: Option<String>,
}

partial_types!(
    PartialTypes,
    tenant_id,
    bucket,
    created_at,
    id,
    email,
    tags,
    custom,
    transient
);

#[charybdis_view_model(
    table_name = types_by_email,
    base_table = types,
    partition_keys = [email],
    clustering_keys = [tenant_id, bucket, created_at, id]
)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TypesByEmail {
    pub email: Text,
    pub tenant_id: Uuid,
    pub bucket: Int,
    pub created_at: Timestamp,
    pub id: Timeuuid,
    pub status: Option<Text>,
}

impl Types {
    /// CQL timestamps have millisecond precision, so the sample uses a value that survives a round trip.
    fn sample_created_at() -> Timestamp {
        chrono::DateTime::from_timestamp_millis(1_700_000_000_123).expect("valid timestamp")
    }

    /// A time based id for the given second, so that tests control how their rows are ordered.
    fn sample_id(unix_seconds: u64) -> Timeuuid {
        Timeuuid::new_v1(uuid::Timestamp::from_unix(uuid::NoContext, unix_seconds, 0), &NODE_ID)
    }

    /// A row with every column populated.
    fn sample(tenant_id: Uuid, bucket: Int, id: Timeuuid) -> Self {
        Types {
            tenant_id,
            bucket,
            created_at: Self::sample_created_at(),
            id,
            tenant_name: Some("tenant".to_string()),
            email: format!("{}@example.com", id),
            status: Some("active".to_string()),
            ascii: "ascii".to_string(),
            big_int: i64::MAX,
            blob: vec![0, 1, 2, 255],
            boolean: true,
            date: chrono::NaiveDate::from_ymd_opt(2024, 2, 29).expect("valid date"),
            decimal: Decimal::from_str("-12345.678900").expect("valid decimal"),
            double: std::f64::consts::PI,
            duration: Some(Duration::new(14, 3, 86_400_000_000_123)),
            float: 1.5,
            inet: Some("2001:db8::1".parse().expect("valid ip address")),
            small_int: i16::MIN,
            time: chrono::NaiveTime::from_hms_nano_opt(23, 59, 59, 123_456_789).expect("valid time"),
            tiny_int: -7,
            varchar: "varchar".to_string(),
            // does not fit into 64 bits
            varint: Varint::from_str("-123456789012345678901234567890").expect("valid integer"),
            tags: HashSet::from(["a".to_string(), "b".to_string()]),
            items: vec![3, 1, 2],
            attrs: HashMap::from([("k1".to_string(), "v1".to_string())]),
            frozen_items: vec!["x".to_string(), "y".to_string()],
            pair: (42, "answer".to_string()),
            nested: Some(HashMap::from([(
                "ids".to_string(),
                vec![Uuid::from_u128(7), Uuid::from_u128(8)],
            )])),
            label: Some(Label {
                name: "first".to_string(),
                weight: Some(1),
                aliases: vec!["one".to_string()],
            }),
            labels: Some(vec![
                Label::default(),
                Label {
                    name: "second".to_string(),
                    weight: None,
                    aliases: vec![],
                },
            ]),
            custom: UserExtraDataCustomField::default(),
            transient: Some("never persisted".to_string()),
        }
    }

    /// What `self` looks like when read back: ignored fields are not persisted and come back as default.
    fn as_persisted(&self) -> Self {
        Types {
            transient: None,
            ..self.clone()
        }
    }
}

#[tokio::test]
async fn round_trip() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let row = Types::sample(Uuid::new_v4(), 1, Types::sample_id(1_700_000_000));
    let persisted = row.as_persisted();

    row.insert().execute(db_session).await?;

    assert_eq!(row.find_by_primary_key().execute(db_session).await?, persisted);

    // the same row through every finder that can address it
    let by_primary_key_value = Types::find_by_primary_key_value(row.primary_key_values())
        .execute(db_session)
        .await?;
    assert_eq!(by_primary_key_value, persisted);

    let by_primary_key =
        Types::find_by_tenant_id_and_bucket_and_created_at_and_id(row.tenant_id, row.bucket, row.created_at, row.id)
            .execute(db_session)
            .await?;
    assert_eq!(by_primary_key, persisted);

    let by_partition_key = Types::find_by_tenant_id_and_bucket(row.tenant_id, row.bucket)
        .execute(db_session)
        .await?
        .try_collect()
        .await?;
    assert_eq!(by_partition_key, vec![persisted.clone()]);

    let first_in_partition = Types::maybe_find_first_by_tenant_id_and_bucket(row.tenant_id, row.bucket)
        .execute(db_session)
        .await?;
    assert_eq!(first_in_partition, Some(persisted));

    row.delete().execute(db_session).await?;

    assert_eq!(row.maybe_find_by_primary_key().execute(db_session).await?, None);
    assert!(matches!(
        row.find_by_primary_key().execute(db_session).await,
        Err(CharybdisError::NotFoundError(_))
    ));

    Ok(())
}

#[tokio::test]
async fn nulls_and_empty_collections_round_trip() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let row = Types {
        tenant_name: None,
        status: None,
        blob: vec![],
        duration: None,
        inet: None,
        tags: HashSet::new(),
        items: vec![],
        attrs: HashMap::new(),
        frozen_items: vec![],
        nested: None,
        label: None,
        labels: None,
        transient: None,
        ..Types::sample(Uuid::new_v4(), 2, Types::sample_id(1_700_000_001))
    };

    row.insert().execute(db_session).await?;

    assert_eq!(row.find_by_primary_key().execute(db_session).await?, row);

    row.delete().execute(db_session).await?;

    Ok(())
}

#[tokio::test]
async fn update_and_static_column() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let tenant_id = Uuid::new_v4();
    let first = Types::sample(tenant_id, 3, Types::sample_id(1_700_000_010));
    let second = Types {
        tenant_name: Some("renamed tenant".to_string()),
        ..Types::sample(tenant_id, 3, Types::sample_id(1_700_000_011))
    };

    first.insert().execute(db_session).await?;
    second.insert().execute(db_session).await?;

    // a static column is shared by every row of the partition
    let first_found = first.find_by_primary_key().execute(db_session).await?;
    assert_eq!(first_found.tenant_name, Some("renamed tenant".to_string()));

    let changed = Types {
        status: Some("archived".to_string()),
        decimal: Decimal::from_str("0.000001").expect("valid decimal"),
        duration: Some(Duration::new(0, 0, 1)),
        pair: (-1, "changed".to_string()),
        items: vec![9],
        label: None,
        ..first_found
    };
    changed.update().execute(db_session).await?;

    assert_eq!(first.find_by_primary_key().execute(db_session).await?, changed);

    // the table is clustered by `created_at DESC, id ASC`, and both rows share `created_at`
    let ids = Types::find_by_tenant_id_and_bucket(tenant_id, 3)
        .execute(db_session)
        .await?
        .try_collect()
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<Timeuuid>>();
    assert_eq!(ids, vec![first.id, second.id]);

    first.delete_by_partition_key().execute(db_session).await?;

    let remaining = Types::find_by_tenant_id_and_bucket(tenant_id, 3)
        .execute(db_session)
        .await?
        .try_collect()
        .await?;
    assert!(remaining.is_empty());

    Ok(())
}

#[tokio::test]
async fn timeuuid_order_matches_clustering_order() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let tenant_id = Uuid::new_v4();
    let rows = [
        1_700_000_500,
        1_600_000_000,
        1_800_000_000,
        1_700_000_499,
        1_650_000_000,
    ]
    .into_iter()
    .map(|unix_seconds| Types::sample(tenant_id, 4, Types::sample_id(unix_seconds)))
    .collect::<Vec<Types>>();

    Types::batch().chunked_insert(db_session, &rows, 2).await?;

    let found_ids = Types::find_by_tenant_id_and_bucket(tenant_id, 4)
        .execute(db_session)
        .await?
        .try_collect()
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<Timeuuid>>();

    // `Timeuuid` must sort the way the database sorts a `timeuuid` clustering column
    let mut sorted_ids = rows.iter().map(|row| row.id).collect::<Vec<Timeuuid>>();
    sorted_ids.sort();
    assert_eq!(found_ids, sorted_ids);

    Types::delete_batch().chunked_delete(db_session, &rows, 2).await?;

    Ok(())
}

#[tokio::test]
async fn collection_helpers() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let row = Types::sample(Uuid::new_v4(), 5, Types::sample_id(1_700_000_020));

    row.insert().execute(db_session).await?;

    row.push_tags(HashSet::from(["c".to_string()]))
        .execute(db_session)
        .await?;
    row.pull_tags(HashSet::from(["a".to_string()]))
        .execute(db_session)
        .await?;
    row.push_items(vec![9, 9]).execute(db_session).await?;
    row.pull_items(vec![1]).execute(db_session).await?;
    row.push_attrs(HashMap::from([("k2".to_string(), "v2".to_string())]))
        .execute(db_session)
        .await?;
    row.pull_attrs(HashSet::from(["k1".to_string()]))
        .execute(db_session)
        .await?;
    row.push_nested(HashMap::from([("more".to_string(), vec![Uuid::from_u128(9)])]))
        .execute(db_session)
        .await?;

    let found = row.find_by_primary_key().execute(db_session).await?;
    assert_eq!(found.tags, HashSet::from(["b".to_string(), "c".to_string()]));
    assert_eq!(found.items, vec![3, 2, 9, 9]);
    assert_eq!(found.attrs, HashMap::from([("k2".to_string(), "v2".to_string())]));
    assert_eq!(
        found.nested,
        Some(HashMap::from([
            ("ids".to_string(), vec![Uuid::from_u128(7), Uuid::from_u128(8)]),
            ("more".to_string(), vec![Uuid::from_u128(9)]),
        ]))
    );

    // the `if_exists` variants apply to an existing row and do not create a missing one
    row.push_tags_if_exists(HashSet::from(["d".to_string()]))
        .execute(db_session)
        .await?;
    row.pull_tags_if_exists(HashSet::from(["b".to_string()]))
        .execute(db_session)
        .await?;

    let missing = Types::sample(row.tenant_id, 5, Types::sample_id(1_700_000_021));
    missing
        .push_tags_if_exists(HashSet::from(["ghost".to_string()]))
        .execute(db_session)
        .await?;

    let found = row.find_by_primary_key().execute(db_session).await?;
    assert_eq!(found.tags, HashSet::from(["c".to_string(), "d".to_string()]));
    assert_eq!(missing.maybe_find_by_primary_key().execute(db_session).await?, None);

    // the generated query constants are usable in statement batches
    let mut batch = Types::statement_batch::<(Set<Text>, Uuid, Int, Timestamp, Timeuuid)>();
    batch.append_statement(
        Types::PUSH_TAGS_QUERY,
        (
            HashSet::from(["batched".to_string()]),
            row.tenant_id,
            row.bucket,
            row.created_at,
            row.id,
        ),
    );
    batch.append_statement(
        Types::PULL_TAGS_QUERY,
        (
            HashSet::from(["c".to_string()]),
            row.tenant_id,
            row.bucket,
            row.created_at,
            row.id,
        ),
    );
    batch.execute(db_session).await?;

    let found = row.find_by_primary_key().execute(db_session).await?;
    assert_eq!(found.tags, HashSet::from(["batched".to_string(), "d".to_string()]));

    row.delete().execute(db_session).await?;

    Ok(())
}

#[tokio::test]
async fn partial_model_and_as_native() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let row = Types::sample(Uuid::new_v4(), 6, Types::sample_id(1_700_000_030));

    row.insert().execute(db_session).await?;

    let mut partial = PartialTypes::find_by_primary_key_value(row.primary_key_values())
        .execute(db_session)
        .await?;
    assert_eq!(partial.email, row.email);
    assert_eq!(partial.tags, row.tags);
    assert_eq!(partial.custom, row.custom);
    assert_eq!(partial.transient, None);

    partial.email = "partial@example.com".to_string();
    partial.custom.user_tags.push(("key".to_string(), "value".to_string()));
    partial.update().execute(db_session).await?;
    partial
        .push_tags(HashSet::from(["from-partial".to_string()]))
        .execute(db_session)
        .await?;

    // writing through the partial model leaves every other column untouched
    let mut expected = Types {
        email: partial.email.clone(),
        custom: partial.custom.clone(),
        ..row.as_persisted()
    };
    expected.tags.insert("from-partial".to_string());

    let native = partial.as_native().find_by_primary_key().execute(db_session).await?;
    assert_eq!(native, expected);

    partial.delete().execute(db_session).await?;

    assert_eq!(row.maybe_find_by_primary_key().execute(db_session).await?, None);

    Ok(())
}

#[tokio::test]
async fn indexes_and_materialized_view() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let row = Types {
        // unique, so that rows left behind by an aborted run cannot show up in the index lookups
        email: format!("{}@example.com", Uuid::new_v4()),
        ..Types::sample(Uuid::new_v4(), 7, Types::sample_id(1_700_000_040))
    };
    let persisted = row.as_persisted();

    row.insert().execute(db_session).await?;

    // global secondary index
    let by_email = eventually(|| async {
        Types::maybe_find_first_by_email(row.email.clone())
            .execute(db_session)
            .await
            .expect("lookup by global secondary index")
    })
    .await;
    assert_eq!(by_email, persisted);

    let all_by_email = Types::find_by_email(row.email.clone())
        .execute(db_session)
        .await?
        .try_collect()
        .await?;
    assert_eq!(all_by_email, vec![persisted.clone()]);

    // local secondary index
    let by_status = eventually(|| async {
        Types::maybe_find_first_by_tenant_id_and_bucket_and_status(row.tenant_id, row.bucket, "active".to_string())
            .execute(db_session)
            .await
            .expect("lookup by local secondary index")
    })
    .await;
    assert_eq!(by_status, persisted);

    // materialized view
    let view_row = eventually(|| async {
        TypesByEmail::maybe_find_first_by_email(row.email.clone())
            .execute(db_session)
            .await
            .expect("lookup in materialized view")
    })
    .await;
    assert_eq!(
        view_row,
        TypesByEmail {
            email: row.email.clone(),
            tenant_id: row.tenant_id,
            bucket: row.bucket,
            created_at: row.created_at,
            id: row.id,
            status: row.status.clone(),
        }
    );

    row.delete().execute(db_session).await?;

    Ok(())
}

#[tokio::test]
async fn query_macros_and_mixed_batch() -> Result<(), CharybdisError> {
    let db_session = &db_session().await;
    let tenant_id = Uuid::new_v4();
    let first = Types::sample(tenant_id, 8, Types::sample_id(1_700_000_050));
    let second = Types::sample(tenant_id, 8, Types::sample_id(1_700_000_051));
    let measurement = Measurement {
        device_id: tenant_id,
        channel: 1,
        metric: "temperature".to_string(),
        taken_at: Types::sample_created_at(),
        is_estimate: Some(true),
        payload: None,
    };

    // one batch over different models and different value shapes
    let mut batch = CharybdisBatch::new();
    batch.append(first.insert());
    batch.append(second.insert());
    batch.append(measurement.insert());
    batch.execute(db_session).await?;

    let ids = find_types!(
        "tenant_id = ? AND bucket = ? AND created_at >= ?",
        (tenant_id, 8, Types::sample_created_at())
    )
    .execute(db_session)
    .await?
    .try_collect()
    .await?
    .into_iter()
    .map(|row| row.id)
    .collect::<Vec<Timeuuid>>();
    assert_eq!(ids, vec![first.id, second.id]);

    let first_found = find_first_types!("tenant_id = ? AND bucket = ? LIMIT 1", (tenant_id, 8))
        .execute(db_session)
        .await?;
    assert_eq!(first_found.id, first.id);

    assert_eq!(
        measurement.find_by_primary_key().execute(db_session).await?,
        measurement
    );

    let mut batch = CharybdisBatch::new();
    batch.append(measurement.delete());
    batch.append(delete_types!("tenant_id = ? AND bucket = ?", (tenant_id, 8)));
    batch.execute(db_session).await?;

    assert_eq!(measurement.maybe_find_by_primary_key().execute(db_session).await?, None);
    assert_eq!(first.maybe_find_by_primary_key().execute(db_session).await?, None);
    assert_eq!(second.maybe_find_by_primary_key().execute(db_session).await?, None);

    Ok(())
}

#[tokio::test]
async fn json_round_trip() -> Result<(), CharybdisError> {
    let row = Types {
        // skipped by serde, so it has to be the default for the comparison below
        transient: None,
        ..Types::sample(Uuid::new_v4(), 9, Types::sample_id(1_700_000_060))
    };

    let json = row.to_json()?;
    assert!(json.contains("\"tenantId\""));
    assert!(json.contains("\"mail\""));

    // numbers that a JSON number cannot be relied on to hold are written as strings
    assert!(json.contains(r#""decimal":"-12345.678900""#));
    assert!(json.contains(r#""varint":"-123456789012345678901234567890""#));

    assert_eq!(Types::from_json(&json)?, row);

    Ok(())
}

use scylla::query::Query;
use scylla::{Bytes, IntoTypedRows};
use scylla::{CachingSession, QueryResult};

use crate::errors::CharybdisError;
use crate::iterator::CharybdisModelIterator;
use crate::model::BaseModel;
use crate::query::CharybdisQuery;
use crate::stream::CharybdisModelStream;
use crate::SerializeRow;

pub trait Find: BaseModel {
    async fn find(
        session: &CachingSession,
        query: &'static str,
        values: impl SerializeRow,
    ) -> Result<CharybdisModelStream<Self>, CharybdisError>;

    async fn find_first(
        session: &CachingSession,
        query: &'static str,
        values: impl SerializeRow,
    ) -> Result<Self, CharybdisError>;

    async fn find_paged(
        session: &CachingSession,
        query: &'static str,
        values: impl SerializeRow,
        page_size: Option<Bytes>,
    ) -> Result<(CharybdisModelIterator<Self>, Option<Bytes>), CharybdisError>;

    async fn find_by_primary_key_value(
        session: &CachingSession,
        value: impl SerializeRow,
    ) -> Result<Self, CharybdisError>;

    async fn find_by_partition_key_value(
        session: &CachingSession,
        value: impl SerializeRow,
    ) -> Result<CharybdisModelStream<Self>, CharybdisError>;

    async fn find_first_by_partition_key_value(
        session: &CachingSession,
        value: impl SerializeRow,
    ) -> Result<Self, CharybdisError>;

    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError>;

    async fn find_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> Result<CharybdisModelStream<Self>, CharybdisError>;
}

impl<T: BaseModel> Find for T {
    // find iter
    async fn find(
        session: &CachingSession,
        query: &'static str,
        values: impl SerializeRow,
    ) -> Result<CharybdisModelStream<Self>, CharybdisError> {
        let query = Query::new(query);

        let rows = session.execute_iter(query, values).await?.into_typed();

        Ok(CharybdisModelStream::from(rows))
    }

    async fn find_first(
        session: &CachingSession,
        query: &'static str,
        values: impl SerializeRow,
    ) -> Result<Self, CharybdisError> {
        let result: QueryResult = session.execute(query, values).await?;
        let typed_row: Self = result.first_row_typed()?;

        Ok(typed_row)
    }

    async fn find_paged(
        session: &CachingSession,
        query: &'static str,
        values: impl SerializeRow,
        paging_state: Option<Bytes>,
    ) -> Result<(CharybdisModelIterator<Self>, Option<Bytes>), CharybdisError> {
        let res = session.execute_paged(query, values, paging_state).await?;
        let paging_state = res.paging_state.clone();
        let rows = res.rows()?;
        let typed_rows = CharybdisModelIterator::from(rows.into_typed());

        Ok((typed_rows, paging_state))
    }

    async fn find_by_primary_key_value(
        session: &CachingSession,
        value: impl SerializeRow,
    ) -> Result<Self, CharybdisError> {
        let result: QueryResult = session.execute(Self::FIND_BY_PRIMARY_KEY_QUERY, value).await?;

        let res = result.first_row_typed()?;

        Ok(res)
    }

    async fn find_by_partition_key_value(
        session: &CachingSession,
        value: impl SerializeRow,
    ) -> Result<CharybdisModelStream<Self>, CharybdisError> {
        let rows = session
            .execute_iter(Self::FIND_BY_PARTITION_KEY_QUERY, value)
            .await?
            .into_typed::<Self>();

        Ok(CharybdisModelStream::from(rows))
    }

    async fn find_first_by_partition_key_value(
        session: &CachingSession,
        value: impl SerializeRow,
    ) -> Result<Self, CharybdisError> {
        let result: QueryResult = session.execute(Self::FIND_BY_PARTITION_KEY_QUERY, value).await?;

        let res = result.first_row_typed()?;

        Ok(res)
    }

    /// Preferred way to find by partition key, as keys will be in correct order
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError> {
        let result: QueryResult = session
            .execute(Self::FIND_BY_PRIMARY_KEY_QUERY, self.primary_key_values())
            .await?;

        let res = result.first_row_typed()?;

        Ok(res)
    }

    /// Preferred way to find by partition key, as keys will be in correct order
    async fn find_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> Result<CharybdisModelStream<Self>, CharybdisError> {
        let rows = session
            .execute_iter(Self::FIND_BY_PARTITION_KEY_QUERY, self.partition_key_values())
            .await?
            .into_typed::<Self>();

        Ok(CharybdisModelStream::from(rows))
    }
}

/// Configurable Find Queries
pub trait FindConfigured<'a>: BaseModel {
    async fn find_cfg(
        query: &'static str,
        values: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, CharybdisModelStream<Self>>, CharybdisError>;

    async fn find_first_cfg(
        query: &'static str,
        values: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, Self>, CharybdisError>;

    async fn find_by_primary_key_value_cfg(
        value: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, Self>, CharybdisError>;

    async fn find_by_partition_key_value_cfg(
        value: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, CharybdisModelStream<Self>>, CharybdisError>;

    async fn find_first_by_partition_key_value_cfg(
        value: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, Self>, CharybdisError>;

    async fn find_by_primary_key_cfg(&self) -> Result<CharybdisQuery<Self>, CharybdisError>;

    async fn find_by_partition_key_cfg(&self) -> Result<CharybdisQuery<CharybdisModelStream<Self>>, CharybdisError>;
}

impl<'a, T: BaseModel> FindConfigured<'a> for T {
    async fn find_cfg(
        query: &'static str,
        values: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, CharybdisModelStream<Self>>, CharybdisError> {
        let query = CharybdisQuery::new(query, values);

        Ok(query)
    }

    async fn find_first_cfg(
        query: &'static str,
        values: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, Self>, CharybdisError> {
        let query = CharybdisQuery::new(query, values);

        Ok(query)
    }

    async fn find_by_primary_key_value_cfg(
        value: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, Self>, CharybdisError> {
        let query = CharybdisQuery::new(Self::FIND_BY_PRIMARY_KEY_QUERY, value);

        Ok(query)
    }

    async fn find_by_partition_key_value_cfg(
        value: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, CharybdisModelStream<Self>>, CharybdisError> {
        let query = CharybdisQuery::new(Self::FIND_BY_PARTITION_KEY_QUERY, value);

        Ok(query)
    }

    async fn find_first_by_partition_key_value_cfg(
        value: impl SerializeRow + 'a,
    ) -> Result<CharybdisQuery<'a, Self>, CharybdisError> {
        let query = CharybdisQuery::new(Self::FIND_BY_PARTITION_KEY_QUERY, value);

        Ok(query)
    }

    async fn find_by_primary_key_cfg(&self) -> Result<CharybdisQuery<Self>, CharybdisError> {
        let query = CharybdisQuery::new(Self::FIND_BY_PRIMARY_KEY_QUERY, self.primary_key_values());

        Ok(query)
    }

    async fn find_by_partition_key_cfg(&self) -> Result<CharybdisQuery<CharybdisModelStream<Self>>, CharybdisError> {
        let query = CharybdisQuery::new(Self::FIND_BY_PARTITION_KEY_QUERY, self.partition_key_values());

        Ok(query)
    }
}

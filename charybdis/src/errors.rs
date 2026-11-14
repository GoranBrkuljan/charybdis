use std::error::Error;
use std::fmt;

use colored::Colorize;
use scylla::deserialize::{DeserializationError, TypeCheckError};
use scylla::errors::{
    ExecutionError, FirstRowError, IntoRowsResultError, MaybeFirstRowError, NextRowError, PagerExecutionError,
    RowsError, SingleRowError,
};

#[derive(Debug)]
pub enum CharybdisError {
    // scylla
    ExecutionError(&'static str, ExecutionError),
    PagerExecutionError(&'static str, PagerExecutionError),
    IntoRowsResultError(&'static str, IntoRowsResultError),
    BatchError(&'static str, ExecutionError),
    SingleRowError(&'static str, SingleRowError),
    RowsError(&'static str, RowsError),
    FirstRowError(&'static str, FirstRowError),
    MaybeFirstRowError(&'static str, MaybeFirstRowError),
    DeserializationError(&'static str, DeserializationError),
    NextRowError(&'static str, NextRowError),
    TypeCheckError(&'static str, TypeCheckError),
    NotFoundError(&'static str),
    JsonError(serde_json::Error),
}

impl fmt::Display for CharybdisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // scylla errors
            CharybdisError::ExecutionError(query, e) => {
                write!(f, "Statement: {}\nExecutionError: {}", query.bright_purple(), e)
            }
            CharybdisError::PagerExecutionError(query, e) => {
                write!(f, "Statement: {}\nPagerExecutionError: {}", query.bright_purple(), e)
            }
            CharybdisError::IntoRowsResultError(query, e) => {
                write!(f, "Statement: {}\nIntoRowsResultError: {}", query.bright_purple(), e)
            }
            CharybdisError::BatchError(query, e) => write!(f, "Model: {}\nBatchError: {}", query.bright_purple(), e),
            CharybdisError::SingleRowError(query, e) => write!(
                f,
                "Statement: {}\nSingleRowError: {:?}. Did you forget to provide complete primary key?",
                query.bright_purple(),
                e
            ),
            CharybdisError::RowsError(query, e) => {
                write!(f, "Statement: {}\nRowsError: {:?}", query.bright_purple(), e)
            }
            CharybdisError::FirstRowError(query, e) => {
                write!(f, "Statement: {}\nFirstRowError: {:?}", query.bright_purple(), e)
            }
            CharybdisError::MaybeFirstRowError(query, e) => {
                write!(f, "Statement: {}\nMaybeFirstRowError: {:?}", query.bright_purple(), e)
            }
            CharybdisError::DeserializationError(query, e) => {
                write!(f, "Statement: {}\nDeserializationError: {:?}", query.bright_purple(), e)
            }
            CharybdisError::NotFoundError(query) => {
                write!(f, "Records not found for query: {}", query.bright_purple())
            }
            CharybdisError::NextRowError(query, e) => {
                write!(f, "Statement: {}\nNextRowError: {:?}", query.bright_purple(), e)
            }
            CharybdisError::TypeCheckError(query, e) => {
                write!(f, "Statement: {}\nTypeCheckError: {:?}", query.bright_purple(), e)
            }
            CharybdisError::JsonError(e) => write!(f, "JsonError: {:?}", e),
        }
    }
}

impl Error for CharybdisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CharybdisError::ExecutionError(_, e) => Some(e),
            CharybdisError::PagerExecutionError(_, e) => Some(e),
            CharybdisError::IntoRowsResultError(_, e) => Some(e),
            CharybdisError::BatchError(_, e) => Some(e),
            CharybdisError::SingleRowError(_, e) => Some(e),
            CharybdisError::RowsError(_, e) => Some(e),
            CharybdisError::FirstRowError(_, e) => Some(e),
            CharybdisError::MaybeFirstRowError(_, e) => Some(e),
            CharybdisError::DeserializationError(_, e) => Some(e),
            CharybdisError::NextRowError(_, e) => Some(e),
            CharybdisError::TypeCheckError(_, e) => Some(e),
            CharybdisError::NotFoundError(_) => None,
            CharybdisError::JsonError(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUERY: &str = "SELECT id FROM users WHERE id = ?";

    #[derive(Debug)]
    struct Cause;

    impl fmt::Display for Cause {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "the cause")
        }
    }

    impl Error for Cause {}

    #[test]
    fn errors_that_wrap_another_error_expose_it_as_source() {
        let json_error = serde_json::from_str::<i32>("not json").expect_err("invalid json");

        let errors = [
            CharybdisError::ExecutionError(QUERY, ExecutionError::EmptyPlan),
            CharybdisError::BatchError("users", ExecutionError::EmptyPlan),
            CharybdisError::DeserializationError(QUERY, DeserializationError::new(Cause)),
            CharybdisError::TypeCheckError(QUERY, TypeCheckError::new(Cause)),
            CharybdisError::JsonError(json_error),
        ];

        for error in errors {
            assert!(error.source().is_some(), "{:?} has no source", error);
        }
    }

    #[test]
    fn not_found_error_has_no_source() {
        assert!(CharybdisError::NotFoundError(QUERY).source().is_none());
    }

    #[test]
    fn display_names_the_statement_or_model() {
        colored::control::set_override(false);

        assert_eq!(
            CharybdisError::NotFoundError(QUERY).to_string(),
            "Records not found for query: SELECT id FROM users WHERE id = ?"
        );
        assert!(
            CharybdisError::ExecutionError(QUERY, ExecutionError::EmptyPlan)
                .to_string()
                .starts_with("Statement: SELECT id FROM users WHERE id = ?\nExecutionError: ")
        );
        assert!(
            CharybdisError::BatchError("users", ExecutionError::EmptyPlan)
                .to_string()
                .starts_with("Model: users\nBatchError: ")
        );
    }
}

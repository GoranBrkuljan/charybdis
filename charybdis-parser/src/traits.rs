use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};

use crate::traits::array::ToStringCollection;
use crate::traits::hash::hash_expr_lit_to_hash;

mod array;
mod hash;
pub mod string;

static EMPTY_VEC: Vec<String> = Vec::new();

#[derive(Debug, Default, Clone)]
pub struct CharybdisMacroArgs {
    pub table_name: Option<String>,
    pub type_name: Option<String>,
    pub base_table: Option<String>,
    pub partition_keys: Option<Vec<String>>,
    pub clustering_keys: Option<Vec<String>>,
    pub static_columns: Option<Vec<String>>,
    pub global_secondary_indexes: Option<Vec<String>>,
    pub local_secondary_indexes: Option<Vec<String>>,
    pub exclude_partial_model: Option<bool>,
    pub fields_names: Option<Vec<String>>,
    pub field_types_hash: Option<HashMap<String, TokenStream>>,
    pub field_attributes_hash: Option<HashMap<String, TokenStream>>,
    pub table_options: Option<String>,
}

impl CharybdisMacroArgs {
    pub fn table_name(&self) -> String {
        self.table_name.clone().expect("table_name is required")
    }

    pub fn partition_keys(&self) -> &Vec<String> {
        self.partition_keys.as_ref().expect("partition_keys is required")
    }

    pub fn clustering_keys(&self) -> &Vec<String> {
        self.clustering_keys.as_ref().expect("clustering_keys is required")
    }

    pub fn static_columns(&self) -> &Vec<String> {
        self.static_columns.as_ref().map_or(&EMPTY_VEC, |x| x)
    }

    pub fn global_secondary_indexes(&self) -> &Vec<String> {
        self.global_secondary_indexes.as_ref().map_or(&EMPTY_VEC, |x| x)
    }

    pub fn local_secondary_indexes(&self) -> &Vec<String> {
        self.local_secondary_indexes.as_ref().map_or(&EMPTY_VEC, |x| x)
    }

    pub fn primary_key(&self) -> Vec<&String> {
        self.partition_keys().iter().chain(self.clustering_keys()).collect()
    }
}

impl Parse for CharybdisMacroArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut type_name = None;
        let mut table_name = None;
        let mut base_table = None;
        let mut partition_keys = None;
        let mut clustering_keys = None;
        let mut static_columns = None;
        let mut global_secondary_indexes = None;
        let mut local_secondary_indexes = None;
        let mut fields_names = None;
        let mut field_types_hash = None;
        let mut field_attributes_hash = None;
        let mut table_options = None;
        let mut exclude_partial_model = None;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;

            match key.to_string().as_str() {
                "type_name" => {
                    let value: syn::Expr = input.parse()?;
                    type_name = Option::from(value.to_token_stream().to_string());
                }
                "table_name" => {
                    let value: syn::Expr = input.parse()?;
                    table_name = Option::from(value.to_token_stream().to_string());
                }
                "base_table" => {
                    let value: syn::Expr = input.parse()?;
                    base_table = Option::from(value.to_token_stream().to_string());
                }
                "partition_keys" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = array.to_vec();

                    partition_keys = Some(parsed)
                }
                "clustering_keys" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = array.to_vec();

                    clustering_keys = Some(parsed)
                }
                "static_columns" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = array.to_vec();

                    static_columns = Some(parsed)
                }
                "global_secondary_indexes" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = array.to_vec();

                    global_secondary_indexes = Some(parsed)
                }
                "local_secondary_indexes" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = array.to_vec();

                    local_secondary_indexes = Some(parsed)
                }
                "exclude_partial_model" => {
                    let value: syn::LitBool = input.parse()?;
                    exclude_partial_model = Option::from(value.value());
                }
                "fields_names" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = array.to_vec();

                    fields_names = Some(parsed)
                }
                "field_types_hash" => {
                    let hash: syn::Expr = input.parse()?;
                    let parsed_field_types_hash = hash_expr_lit_to_hash(hash, "field_types_hash".to_string());

                    field_types_hash = Some(parsed_field_types_hash);
                }
                "field_attributes_hash" => {
                    // parse ruby style hash
                    let hash: syn::Expr = input.parse()?;
                    let parsed_field_attributes_hash = hash_expr_lit_to_hash(hash, "field_attributes_hash".to_string());
                    field_attributes_hash = Some(parsed_field_attributes_hash);
                }
                "table_options" => {
                    // parse ruby style hash
                    let value: syn::LitStr = input.parse()?;
                    table_options = Option::from(value.value());
                }
                _ => {}
            }

            if !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
            }
        }

        Ok(CharybdisMacroArgs {
            type_name,
            table_name,
            base_table,
            partition_keys,
            clustering_keys,
            static_columns,
            global_secondary_indexes,
            local_secondary_indexes,
            fields_names,
            field_types_hash,
            field_attributes_hash,
            table_options,
            exclude_partial_model,
        })
    }
}

impl From<TokenStream> for CharybdisMacroArgs {
    fn from(tokens: TokenStream) -> Self {
        // Convert the input tokens to a ParseStream
        let parse_stream: TokenStream = syn::parse2(tokens).unwrap();

        // Parse the ParseStream into a MyStruct instance
        let my_struct: CharybdisMacroArgs = syn::parse2(parse_stream).unwrap();

        // Return the parsed MyStruct instance
        my_struct
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &str) -> CharybdisMacroArgs {
        syn::parse_str(args).expect("valid macro arguments")
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    /// Token streams print with spaces between tokens; the spacing is not significant.
    fn compact(tokens: &TokenStream) -> String {
        tokens.to_string().replace(' ', "")
    }

    #[test]
    fn parses_table_arguments() {
        let args = parse(
            r#"
                table_name = users,
                partition_keys = [tenant_id, id],
                clustering_keys = [created_at],
                static_columns = [tenant_name],
                global_secondary_indexes = [email],
                local_secondary_indexes = [status, kind],
                table_options = "gc_grace_seconds = 86400",
                exclude_partial_model = true,
            "#,
        );

        assert_eq!(args.table_name(), "users");
        assert_eq!(args.partition_keys(), &strings(&["tenant_id", "id"]));
        assert_eq!(args.clustering_keys(), &strings(&["created_at"]));
        assert_eq!(args.static_columns(), &strings(&["tenant_name"]));
        assert_eq!(args.global_secondary_indexes(), &strings(&["email"]));
        assert_eq!(args.local_secondary_indexes(), &strings(&["status", "kind"]));
        assert_eq!(args.table_options.as_deref(), Some("gc_grace_seconds = 86400"));
        assert_eq!(args.exclude_partial_model, Some(true));
    }

    #[test]
    fn primary_key_is_the_partition_key_followed_by_the_clustering_key() {
        let args = parse("table_name = users, partition_keys = [tenant_id, id], clustering_keys = [created_at, kind]");
        let primary_key = args.primary_key().into_iter().cloned().collect::<Vec<String>>();

        assert_eq!(primary_key, strings(&["tenant_id", "id", "created_at", "kind"]));
    }

    #[test]
    fn optional_arguments_default_to_nothing() {
        let args = parse("table_name = users, partition_keys = [id], clustering_keys = []");

        assert!(args.clustering_keys().is_empty());
        assert!(args.static_columns().is_empty());
        assert!(args.global_secondary_indexes().is_empty());
        assert!(args.local_secondary_indexes().is_empty());
        assert_eq!(args.table_options, None);
        assert_eq!(args.exclude_partial_model, None);
        assert_eq!(args.base_table, None);
        assert_eq!(args.type_name, None);
    }

    #[test]
    fn parses_materialized_view_arguments() {
        let args = parse("table_name=users_by_email, base_table=users, partition_keys=[email], clustering_keys=[id]");

        assert_eq!(args.table_name(), "users_by_email");
        assert_eq!(args.base_table.as_deref(), Some("users"));
        assert_eq!(args.partition_keys(), &strings(&["email"]));
        assert_eq!(args.clustering_keys(), &strings(&["id"]));
    }

    #[test]
    fn parses_udt_arguments() {
        let args = parse("type_name = address");

        assert_eq!(args.type_name.as_deref(), Some("address"));
        assert_eq!(args.table_name, None);
    }

    #[test]
    fn table_options_can_be_a_raw_string() {
        let args = parse(
            r##"
                table_name = users,
                partition_keys = [id],
                clustering_keys = [created_at],
                table_options = r#"
                    CLUSTERING ORDER BY (created_at DESC)
                "#
            "##,
        );

        assert_eq!(
            args.table_options.as_deref().map(str::trim),
            Some("CLUSTERING ORDER BY (created_at DESC)")
        );
    }

    #[test]
    fn unknown_argument_is_an_error() {
        assert!(syn::parse_str::<CharybdisMacroArgs>("tabel_name = users, partition_keys = [id]").is_err());
    }

    // these are passed from `partial_<model>!` to the macro that builds the partial model
    #[test]
    fn parses_partial_model_arguments() {
        let args = parse(
            r#"
                fields_names = [id, username],
                field_types_hash = "id => Uuid ; username => Option < Text > ;",
                field_attributes_hash = "id => ; username => # [serde (rename = \"name\")] ;"
            "#,
        );

        assert_eq!(args.fields_names, Some(strings(&["id", "username"])));

        let types = args.field_types_hash.expect("field types");
        assert_eq!(types.len(), 2);
        assert_eq!(compact(&types["id"]), "Uuid");
        assert_eq!(compact(&types["username"]), "Option<Text>");

        let attributes = args.field_attributes_hash.expect("field attributes");
        assert_eq!(attributes.len(), 2);
        assert_eq!(compact(&attributes["id"]), "");
        assert_eq!(compact(&attributes["username"]), "#[serde(rename=\"name\")]");
    }
}

use std::fmt::Display;
use std::path::PathBuf;

use colored::Colorize;
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

use crate::schema::{SchemaObject, SchemaObjects};

mod parser;

#[derive(Eq, PartialEq)]
pub(crate) enum ModelMacro {
    Table,
    Udt,
    MaterializedView,
}

impl Display for ModelMacro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelMacro::Table => write!(f, "charybdis_model"),
            ModelMacro::MaterializedView => write!(f, "charybdis_view_model"),
            ModelMacro::Udt => write!(f, "charybdis_udt_model"),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct CodeSchema {
    pub tables: SchemaObjects,
    pub udts: SchemaObjects,
    pub materialized_views: SchemaObjects,
}

impl CodeSchema {
    pub fn new(current_dir: &String) -> CodeSchema {
        let mut current_code_schema = CodeSchema {
            tables: SchemaObjects::new(),
            udts: SchemaObjects::new(),
            materialized_views: SchemaObjects::new(),
        };

        current_code_schema.get_models_from_code(current_dir);

        current_code_schema
    }

    pub fn get_models_from_code(&mut self, current_dir: &String) {
        let current_dir: PathBuf = PathBuf::from(current_dir);
        let walker = WalkDir::new(&current_dir)
            .into_iter()
            .filter_entry(|e| !(e.file_type().is_dir() && e.file_name() == "target"));

        for entry in walker {
            // ignore target directory
            let entry: DirEntry = entry.unwrap();

            if entry.path().is_file() {
                let path = entry.path().to_str().unwrap();

                if !path.ends_with(".rs") {
                    continue;
                }

                let file_content: String = parser::parse_file_as_string(entry.path());
                let ast: syn::File = syn::parse_file(&file_content)
                    .inspect_err(|_e| {
                        println!(
                            "{}\n",
                            format!("Error parsing file: {}", file_content).bright_red().bold()
                        );
                    })
                    .unwrap();

                self.populate_materialized_views(&ast);
                self.populate_udts(&ast);
                self.populate_tables(&ast);
            }
        }
    }

    pub fn populate_materialized_views(&mut self, ast: &syn::File) {
        let schema_objects: Vec<SchemaObject> = parser::parse_charybdis_model_def(ast, ModelMacro::MaterializedView);

        schema_objects.into_iter().for_each(|schema_object| {
            self.materialized_views
                .insert(schema_object.table_name.clone(), schema_object);
        });
    }

    pub fn populate_udts(&mut self, ast: &syn::File) {
        let schema_objects: Vec<SchemaObject> = parser::parse_charybdis_model_def(ast, ModelMacro::Udt);

        schema_objects.into_iter().for_each(|schema_object| {
            self.udts.insert(schema_object.type_name.to_lowercase(), schema_object);
        });
    }

    pub fn populate_tables(&mut self, ast: &syn::File) {
        let schema_object: Vec<SchemaObject> = parser::parse_charybdis_model_def(ast, ModelMacro::Table);

        schema_object.into_iter().for_each(|schema_object| {
            self.tables.insert(schema_object.table_name.clone(), schema_object);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
        #[derive(Default)]
        #[charybdis_udt_model(type_name = address)]
        pub struct Address {
            pub street: Text,
            pub city: Option<Text>,
        }

        #[charybdis_model(
            table_name = users,
            partition_keys = [tenant_id, id],
            clustering_keys = [created_at],
            static_columns = [tenant_name],
            global_secondary_indexes = [email],
            local_secondary_indexes = [status],
            table_options = "gc_grace_seconds = 86400"
        )]
        #[derive(Default)]
        pub struct User {
            pub tenant_id: Uuid,
            pub id: Uuid,
            pub created_at: Timestamp,
            pub tenant_name: Option<Text>,
            pub email: Text,
            pub status: Option<Text>,
            pub tags: Option<Set<Text>>,
            pub address: Frozen<Address>,
            #[charybdis(column_type = "Text")]
            pub settings: Settings,
            #[charybdis(ignore)]
            pub cache: Option<Session>,
        }

        #[charybdis_view_model(
            table_name = users_by_email,
            base_table = users,
            partition_keys = [email],
            clustering_keys = [tenant_id, id, created_at]
        )]
        pub struct UsersByEmail {
            pub email: Text,
            pub tenant_id: Uuid,
            pub id: Uuid,
            pub created_at: Timestamp,
        }

        pub struct NotAModel {
            pub id: Uuid,
        }

        impl User {
            pub fn is_active(&self) -> bool {
                self.status.as_deref() == Some("active")
            }
        }
    "#;

    fn schema_of(source: &str) -> CodeSchema {
        let ast = syn::parse_file(source).expect("valid source file");
        let mut schema = CodeSchema::default();

        schema.populate_udts(&ast);
        schema.populate_tables(&ast);
        schema.populate_materialized_views(&ast);

        schema
    }

    /// `(column, type, is static)`. Generic types are printed with spaces between the tokens, which
    /// are not significant, so they are removed.
    fn fields(schema_object: &SchemaObject) -> Vec<(String, String, bool)> {
        schema_object
            .fields
            .iter()
            .map(|(name, field_type, is_static)| (name.clone(), field_type.replace(' ', ""), *is_static))
            .collect()
    }

    fn field(name: &str, field_type: &str, is_static: bool) -> (String, String, bool) {
        (name.to_string(), field_type.to_string(), is_static)
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn only_structs_with_a_model_macro_are_collected() {
        let schema = schema_of(SOURCE);

        assert_eq!(schema.tables.keys().collect::<Vec<_>>(), ["users"]);
        assert_eq!(schema.udts.keys().collect::<Vec<_>>(), ["address"]);
        assert_eq!(schema.materialized_views.keys().collect::<Vec<_>>(), ["users_by_email"]);
    }

    #[test]
    fn table_columns_keys_indexes_and_options_are_read() {
        let schema = schema_of(SOURCE);
        let users = &schema.tables["users"];

        assert_eq!(
            fields(users),
            [
                field("tenant_id", "Uuid", false),
                field("id", "Uuid", false),
                field("created_at", "Timestamp", false),
                // a static column, and `Option` is not part of the column type
                field("tenant_name", "Text", true),
                field("email", "Text", false),
                field("status", "Text", false),
                field("tags", "Set<Text>", false),
                field("address", "Frozen<Address>", false),
                // the `column_type` attribute wins over the type of the field
                field("settings", "Text", false),
                // and the ignored field `cache` is not a column at all
            ]
        );
        assert!(users.contains_field("settings"));
        assert!(!users.contains_field("cache"));
        assert_eq!(users.types_by_name["email"], "Text");

        assert_eq!(users.table_name, "users");
        assert_eq!(users.partition_keys, strings(&["tenant_id", "id"]));
        assert_eq!(users.clustering_keys, strings(&["created_at"]));
        assert_eq!(users.static_columns, strings(&["tenant_name"]));
        // indexes get their name when they are created
        assert_eq!(users.global_secondary_indexes, [(String::new(), "email".to_string())]);
        assert_eq!(users.local_secondary_indexes, [(String::new(), "status".to_string())]);
        assert_eq!(users.table_options.as_deref(), Some("gc_grace_seconds = 86400"));
    }

    #[test]
    fn udt_fields_are_read() {
        let schema = schema_of(SOURCE);
        let address = &schema.udts["address"];

        assert_eq!(address.type_name, "address");
        assert_eq!(
            fields(address),
            [field("street", "Text", false), field("city", "Text", false)]
        );
    }

    #[test]
    fn materialized_view_columns_and_keys_are_read() {
        let schema = schema_of(SOURCE);
        let view = &schema.materialized_views["users_by_email"];

        assert_eq!(view.table_name, "users_by_email");
        assert_eq!(view.base_table, "users");
        assert_eq!(view.partition_keys, strings(&["email"]));
        assert_eq!(view.clustering_keys, strings(&["tenant_id", "id", "created_at"]));
        assert_eq!(
            fields(view),
            [
                field("email", "Text", false),
                field("tenant_id", "Uuid", false),
                field("id", "Uuid", false),
                field("created_at", "Timestamp", false),
            ]
        );
    }

    #[test]
    fn udt_names_are_lowercased() {
        let schema =
            schema_of("#[charybdis_udt_model(type_name = ReorderData)] pub struct ReorderData { pub id: Uuid }");

        assert_eq!(schema.udts.keys().collect::<Vec<_>>(), ["reorderdata"]);
    }

    #[test]
    #[should_panic(expected = "Partition keys are required in charybdis_model macro")]
    fn table_needs_partition_keys() {
        schema_of("#[charybdis_model(table_name = users)] pub struct User { pub id: Uuid }");
    }

    #[test]
    #[should_panic(expected = "Table name is required in charybdis_model macro")]
    fn table_needs_a_name() {
        schema_of("#[charybdis_model(partition_keys = [id])] pub struct User { pub id: Uuid }");
    }

    #[test]
    #[should_panic(expected = "Base table is required in charybdis_view_model macro")]
    fn materialized_view_needs_a_base_table() {
        schema_of(
            "#[charybdis_view_model(table_name = users_by_email, partition_keys = [email])] \
             pub struct UsersByEmail { pub email: Text }",
        );
    }

    #[test]
    #[should_panic(expected = "Type name is required in charybdis_udt_model macro")]
    fn udt_needs_a_type_name() {
        schema_of("#[charybdis_udt_model()] pub struct Address { pub street: Text }");
    }
}

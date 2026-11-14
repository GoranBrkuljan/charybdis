use charybdis_parser::schema::{IndexName, SchemaObject};

use crate::model::ModelType;
use crate::model::runner::INDEX_SUFFIX;

type FieldName = String;
type FieldType = String;
type OldFieldType = FieldType;
type NewFieldType = FieldType;

pub struct ModelData<'a> {
    pub(crate) migration_object_name: &'a String,
    pub(crate) migration_object_type: ModelType,
    pub(crate) current_code_schema: &'a SchemaObject,
    pub(crate) current_db_schema: &'a SchemaObject,
    pub(crate) new_fields: Vec<(FieldName, FieldType)>,
    pub(crate) removed_fields: Vec<FieldName>,
    pub(crate) new_global_secondary_indexes: Vec<FieldName>,
    pub(crate) new_local_secondary_indexes: Vec<FieldName>,
    pub(crate) removed_global_secondary_indexes: Vec<IndexName>,
    pub(crate) removed_local_secondary_indexes: Vec<IndexName>,
    pub(crate) changed_field_types: Vec<(FieldName, OldFieldType, NewFieldType)>,
}

impl<'a> ModelData<'a> {
    pub(crate) fn new(
        migration_object_name: &'a String,
        migration_object_type: ModelType,
        current_code_schema: &'a SchemaObject,
        current_db_schema: &'a SchemaObject,
    ) -> Self {
        let mut data = Self {
            migration_object_name,
            migration_object_type,
            current_code_schema,
            current_db_schema,
            new_fields: vec![],
            removed_fields: vec![],
            new_global_secondary_indexes: vec![],
            new_local_secondary_indexes: vec![],
            removed_global_secondary_indexes: vec![],
            removed_local_secondary_indexes: vec![],
            changed_field_types: vec![],
        };

        data.fetch_new_fields();
        data.fetch_removed_fields();
        data.fetch_new_global_secondary_indexes();
        data.fetch_removed_global_secondary_indexes();
        data.fetch_new_local_secondary_indexes();
        data.fetch_removed_local_secondary_indexes();
        data.fetch_changed_field_types();

        data
    }

    pub(crate) fn construct_index_name(&self, column_name: &String) -> String {
        format!("{}_{}_{}", self.migration_object_name, column_name, INDEX_SUFFIX)
    }

    pub(crate) fn is_first_migration(&self) -> bool {
        self.current_db_schema.fields.is_empty()
    }

    pub(crate) fn has_new_global_secondary_indexes(&self) -> bool {
        !self.new_global_secondary_indexes.is_empty()
    }

    pub(crate) fn has_removed_global_secondary_indexes(&self) -> bool {
        !self.removed_global_secondary_indexes.is_empty()
    }

    pub(crate) fn has_new_local_secondary_indexes(&self) -> bool {
        !self.new_local_secondary_indexes.is_empty()
    }

    pub(crate) fn has_removed_local_secondary_indexes(&self) -> bool {
        !self.removed_local_secondary_indexes.is_empty()
    }

    pub(crate) fn has_new_fields(&self) -> bool {
        !self.new_fields.is_empty()
    }

    pub(crate) fn has_removed_fields(&self) -> bool {
        !self.removed_fields.is_empty()
    }

    // Checks if any field of db schema has changed type in code schema.
    pub(crate) fn has_changed_type_fields(&self) -> bool {
        !self.changed_field_types.is_empty()
    }

    pub(crate) fn partition_key_changed(&self) -> bool {
        let mut code_partition_keys = self.current_code_schema.partition_keys.clone();
        let mut db_partition_keys = self.current_db_schema.partition_keys.clone();

        code_partition_keys.sort();
        db_partition_keys.sort();

        code_partition_keys != db_partition_keys
    }

    pub(crate) fn clustering_key_changed(&self) -> bool {
        let mut code_clustering_keys = self.current_code_schema.clustering_keys.clone();
        let mut db_clustering_keys = self.current_db_schema.clustering_keys.clone();

        code_clustering_keys.sort();
        db_clustering_keys.sort();

        code_clustering_keys != db_clustering_keys
    }

    fn fetch_new_fields(&mut self) {
        for (field_name, field_type, _) in self.current_code_schema.fields.iter() {
            if !self.current_db_schema.contains_field(field_name) {
                self.new_fields.push((field_name.clone(), field_type.clone()));
            }
        }
    }

    fn fetch_removed_fields(&mut self) {
        for (field_name, _, _) in self.current_db_schema.fields.iter() {
            if !self.current_code_schema.contains_field(field_name) {
                self.removed_fields.push(field_name.clone());
            }
        }
    }

    fn fetch_new_global_secondary_indexes(&mut self) {
        let _ = &self
            .current_code_schema
            .global_secondary_indexes
            .iter()
            .for_each(|(_index_name, index_target)| {
                if !self
                    .current_db_schema
                    .global_secondary_indexes
                    .iter()
                    .any(|(_, target)| target == index_target)
                {
                    self.new_global_secondary_indexes.push(index_target.clone());
                }
            });
    }

    fn fetch_removed_global_secondary_indexes(&mut self) {
        let _ = &self
            .current_db_schema
            .global_secondary_indexes
            .iter()
            .for_each(|(index_name, index_target)| {
                if !self
                    .current_code_schema
                    .global_secondary_indexes
                    .iter()
                    .any(|(_index_name, target)| target == index_target)
                {
                    self.removed_global_secondary_indexes.push(index_name.clone());
                }
            });
    }

    fn fetch_new_local_secondary_indexes(&mut self) {
        let _ = &self
            .current_code_schema
            .local_secondary_indexes
            .iter()
            .for_each(|(_index_name, index_target)| {
                if !self
                    .current_db_schema
                    .local_secondary_indexes
                    .iter()
                    .any(|(_, target)| target == index_target)
                {
                    self.new_local_secondary_indexes.push(index_target.clone());
                }
            });
    }

    fn fetch_removed_local_secondary_indexes(&mut self) {
        let _ = &self
            .current_db_schema
            .local_secondary_indexes
            .iter()
            .for_each(|(index_name, index_target)| {
                if !self
                    .current_code_schema
                    .local_secondary_indexes
                    .iter()
                    .any(|(_index_name, target)| target == index_target)
                {
                    self.removed_local_secondary_indexes.push(index_name.clone());
                }
            });
    }

    fn fetch_changed_field_types(&mut self) {
        for (field_name, field_type, _) in self.current_code_schema.fields.iter() {
            if let Some(db_field_type) = self.current_db_schema.types_by_name.get(field_name) {
                let code_field_type = field_type.to_lowercase().replace(' ', "");
                let db_field_type = db_field_type.to_lowercase().replace(' ', "");

                if resolve_type_aliases(&code_field_type) != resolve_type_aliases(&db_field_type) {
                    self.changed_field_types
                        .push((field_name.clone(), db_field_type, code_field_type));
                }
            }
        }
    }
}

/// Replaces every type name that is an alias with the name the database reports for it, so that a
/// column is not seen as changed only because the model spells its type differently.
///
/// Expects a lowercase type without spaces, e.g. `map<varchar,frozen<list<int>>>`.
fn resolve_type_aliases(field_type: &str) -> String {
    let mut resolved = String::with_capacity(field_type.len());
    let mut type_name = String::new();

    for character in field_type.chars() {
        if character.is_alphanumeric() || character == '_' {
            type_name.push(character);
        } else {
            resolved.push_str(canonical_type_name(&type_name));
            resolved.push(character);
            type_name.clear();
        }
    }

    resolved.push_str(canonical_type_name(&type_name));

    resolved
}

fn canonical_type_name(type_name: &str) -> &str {
    match type_name {
        // `varchar` is an alias of `text`, and the database reports such columns as `text`
        "varchar" => "text",
        _ => type_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_name() -> String {
        "users".to_string()
    }

    /// A schema with the given `(column, type)` pairs, filled in the way the code parser and the
    /// database reader fill it.
    fn schema(fields: &[(&str, &str)]) -> SchemaObject {
        let mut schema = SchemaObject::default();

        for (name, field_type) in fields {
            schema.fields.push((name.to_string(), field_type.to_string(), false));
            schema.field_names.insert(name.to_string());
            schema.types_by_name.insert(name.to_string(), field_type.to_string());
        }

        schema
    }

    fn index(name: &str, column: &str) -> (IndexName, String) {
        (name.to_string(), column.to_string())
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn empty_database_schema_means_first_migration() {
        let name = table_name();
        let code = schema(&[("id", "Uuid")]);
        let empty = SchemaObject::default();

        assert!(ModelData::new(&name, ModelType::Table, &code, &empty).is_first_migration());
        assert!(!ModelData::new(&name, ModelType::Table, &code, &code).is_first_migration());
    }

    #[test]
    fn matching_schemas_have_no_changes() {
        let name = table_name();
        let code = schema(&[("id", "Uuid"), ("username", "Text")]);
        let db = schema(&[("id", "uuid"), ("username", "text")]);

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        assert!(!data.has_new_fields());
        assert!(!data.has_removed_fields());
        assert!(!data.has_changed_type_fields());
        assert!(!data.has_new_global_secondary_indexes());
        assert!(!data.has_removed_global_secondary_indexes());
        assert!(!data.has_new_local_secondary_indexes());
        assert!(!data.has_removed_local_secondary_indexes());
        assert!(!data.partition_key_changed());
        assert!(!data.clustering_key_changed());
    }

    #[test]
    fn detects_new_and_removed_fields() {
        let name = table_name();
        let code = schema(&[("id", "Uuid"), ("username", "Text"), ("bio", "Text")]);
        let db = schema(&[("id", "uuid"), ("username", "text"), ("legacy", "int")]);

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        assert_eq!(data.new_fields, vec![("bio".to_string(), "Text".to_string())]);
        assert_eq!(data.removed_fields, strings(&["legacy"]));
        assert!(!data.has_changed_type_fields());
    }

    #[test]
    fn detects_changed_field_types() {
        let name = table_name();
        let code = schema(&[("id", "Uuid"), ("age", "BigInt")]);
        let db = schema(&[("id", "uuid"), ("age", "int")]);

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        // (column, type in the database, type in the code)
        assert_eq!(
            data.changed_field_types,
            vec![("age".to_string(), "int".to_string(), "bigint".to_string())]
        );
    }

    #[test]
    fn type_comparison_ignores_case_and_spacing() {
        let name = table_name();
        // the code parser prints generic types with spaces around every token
        let code = schema(&[("tags", "Map < Text , Frozen < List < Uuid > > >")]);
        let db = schema(&[("tags", "map<text, frozen<list<uuid>>>")]);

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        assert!(!data.has_changed_type_fields());
    }

    #[test]
    fn varchar_is_the_same_type_as_text() {
        let name = table_name();
        // the database reports `varchar` columns as `text`
        let code = schema(&[("nickname", "Varchar"), ("aliases", "Frozen < List < Varchar > >")]);
        let db = schema(&[("nickname", "text"), ("aliases", "frozen<list<text>>")]);

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        assert_eq!(data.changed_field_types, vec![]);
    }

    #[test]
    fn a_type_that_only_contains_varchar_in_its_name_is_not_an_alias() {
        let name = table_name();
        let code = schema(&[("label", "VarcharLabel")]);
        let db = schema(&[("label", "textlabel")]);

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        assert_eq!(
            data.changed_field_types,
            vec![("label".to_string(), "textlabel".to_string(), "varcharlabel".to_string())]
        );
    }

    #[test]
    fn detects_new_and_removed_global_secondary_indexes() {
        let name = table_name();
        let mut code = schema(&[("id", "Uuid"), ("username", "Text"), ("email", "Text")]);
        let mut db = schema(&[("id", "uuid"), ("username", "text"), ("email", "text")]);
        // indexes declared in the code have no name yet
        code.global_secondary_indexes = vec![index("", "username"), index("", "email")];
        db.global_secondary_indexes = vec![index("users_username_idx", "username"), index("users_id_idx", "id")];

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        // new indexes are reported by column, removed ones by the name they have in the database
        assert_eq!(data.new_global_secondary_indexes, strings(&["email"]));
        assert_eq!(data.removed_global_secondary_indexes, strings(&["users_id_idx"]));
        assert!(!data.has_new_local_secondary_indexes());
        assert!(!data.has_removed_local_secondary_indexes());
    }

    #[test]
    fn detects_new_and_removed_local_secondary_indexes() {
        let name = table_name();
        let mut code = schema(&[("id", "Uuid"), ("username", "Text"), ("email", "Text")]);
        let mut db = schema(&[("id", "uuid"), ("username", "text"), ("email", "text")]);
        code.local_secondary_indexes = vec![index("", "username"), index("", "email")];
        db.local_secondary_indexes = vec![
            index("users_id_username_idx", "username"),
            index("users_id_bio_idx", "bio"),
        ];

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        assert_eq!(data.new_local_secondary_indexes, strings(&["email"]));
        assert_eq!(data.removed_local_secondary_indexes, strings(&["users_id_bio_idx"]));
        assert!(!data.has_new_global_secondary_indexes());
        assert!(!data.has_removed_global_secondary_indexes());
    }

    #[test]
    fn detects_partition_and_clustering_key_changes() {
        let name = table_name();
        let mut code = schema(&[("id", "Uuid"), ("tenant_id", "Uuid"), ("created_at", "Timestamp")]);
        let mut db = schema(&[("id", "uuid"), ("tenant_id", "uuid"), ("created_at", "timestamp")]);
        code.partition_keys = strings(&["tenant_id", "id"]);
        db.partition_keys = strings(&["id"]);
        code.clustering_keys = strings(&[]);
        db.clustering_keys = strings(&["created_at"]);

        let data = ModelData::new(&name, ModelType::Table, &code, &db);

        assert!(data.partition_key_changed());
        assert!(data.clustering_key_changed());
    }

    #[test]
    fn index_name_is_built_from_the_table_and_the_column() {
        let name = table_name();
        let code = schema(&[("id", "Uuid")]);

        let data = ModelData::new(&name, ModelType::Table, &code, &code);

        assert_eq!(data.construct_index_name(&"username".to_string()), "users_username_idx");
    }
}

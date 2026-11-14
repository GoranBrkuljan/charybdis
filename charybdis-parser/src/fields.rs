use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use darling::FromAttributes;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Fields, FieldsNamed, GenericArgument, PathArguments, Type};

use crate::traits::CharybdisMacroArgs;

#[derive(Clone, PartialEq, strum_macros::Display, strum_macros::EnumString)]
pub enum CqlType {
    Ascii,
    BigInt,
    Blob,
    Boolean,
    Counter,
    Date,
    Decimal,
    Double,
    Duration,
    Float,
    Inet,
    Int,
    SmallInt,
    Text,
    Time,
    Timestamp,
    Timeuuid,
    TinyInt,
    Uuid,
    Varchar,
    Varint,
    Map,
    List,
    Set,
    Tuple,
    Ignored,
    /// This is used when the type is not recognized. In future we might want to extend recognition to UDTs,
    /// so we can panic if type is not recognized.
    Unknown,
}

#[derive(FromAttributes, Clone)]
#[darling(attributes(charybdis))]
pub struct FieldAttributes {
    #[darling(default)]
    pub ignore: Option<bool>,

    #[darling(default)]
    pub column_type: Option<String>,
}

pub struct Field<'a> {
    pub name: String,
    pub ident: syn::Ident,
    pub ty: Type,
    pub ty_path: syn::TypePath,
    pub outer_type: CqlType,
    pub column_type_override: Option<String>,
    pub span: proc_macro2::Span,
    pub attrs: &'a Vec<syn::Attribute>,
    pub ignore: bool,
    pub is_partition_key: bool,
    pub is_clustering_key: bool,
    pub is_static_column: bool,
}

impl<'a> Field<'a> {
    pub fn outer_type(ty: &Type, ignore: bool) -> CqlType {
        if ignore {
            return CqlType::Ignored;
        }

        if let Type::Path(type_path) = ty {
            // Handle the outer type being an Option
            if let Some(last_segment) = type_path.path.segments.last() {
                if last_segment.ident == "Option" {
                    if let PathArguments::AngleBracketed(args) = &last_segment.arguments
                        && let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first()
                    {
                        let inner = inner_type_path.path.segments.last().expect("No inner type found");

                        return inner
                            .ident
                            .to_string()
                            .parse::<CqlType>()
                            .ok()
                            .unwrap_or(CqlType::Unknown);
                    }
                } else {
                    return last_segment
                        .ident
                        .to_string()
                        .parse::<CqlType>()
                        .ok()
                        .unwrap_or(CqlType::Unknown);
                }
            }

            panic!("Unable to parse type for field: {:?}", ty);
        }

        panic!("Unable to parse type for field: {:?}", ty);
    }

    pub fn from_field(
        field: &'a syn::Field,
        is_partition_key: bool,
        is_clustering_key: bool,
        is_static_column: bool,
    ) -> Self {
        FieldAttributes::from_attributes(&field.attrs)
            .map(|char_attrs| {
                let ignore = char_attrs.ignore.unwrap_or(false);
                let column_type =
                    char_attrs
                        .column_type
                        .as_ref()
                        .map_or(Field::outer_type(&field.ty, ignore), |tname| {
                            let error = format!("Unknown column type: {}", tname);
                            CqlType::from_str(tname.as_str()).expect(&error)
                        });
                let ident = field.ident.clone().unwrap();

                Field {
                    name: ident.to_string(),
                    ident,
                    ty: field.ty.clone(),
                    ty_path: match &field.ty {
                        Type::Path(type_path) => type_path.clone(),
                        _ => panic!("Only type path is supported!"),
                    },
                    outer_type: column_type,
                    column_type_override: char_attrs.column_type,
                    span: field.span(),
                    attrs: &field.attrs,
                    ignore,
                    is_partition_key,
                    is_clustering_key,
                    is_static_column,
                }
            })
            .unwrap()
    }

    pub fn is_primary_key(&self) -> bool {
        self.is_partition_key || self.is_clustering_key
    }

    pub fn is_list(&self) -> bool {
        self.outer_type == CqlType::List
    }

    pub fn is_set(&self) -> bool {
        self.outer_type == CqlType::Set
    }

    pub fn is_map(&self) -> bool {
        self.outer_type == CqlType::Map
    }

    pub fn is_collection(&self) -> bool {
        self.is_list() || self.is_set() || self.is_map()
    }

    pub fn is_counter(&self) -> bool {
        self.outer_type == CqlType::Counter
    }

    pub fn is_tuple(&self) -> bool {
        self.outer_type == CqlType::Tuple
    }

    pub fn is_frozen(&self) -> bool {
        self.ty_path
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == "Frozen")
    }
}

#[derive(Default)]
pub struct CharybdisFields<'a> {
    pub all_fields: Vec<Field<'a>>,
    pub partition_key_fields: Vec<&'a Field<'a>>,
    pub clustering_key_fields: Vec<&'a Field<'a>>,
    pub primary_key_fields: Vec<&'a Field<'a>>,
    pub db_fields: Vec<&'a Field<'a>>,
    pub global_secondary_index_fields: Vec<&'a Field<'a>>,
    pub local_secondary_index_fields: Vec<&'a Field<'a>>,
}

impl<'a> CharybdisFields<'a> {
    fn new(named_fields: &'a FieldsNamed, args: &CharybdisMacroArgs) -> Self {
        let mut me = Self::default();

        for field in &named_fields.named {
            let field_name = field.ident.clone().expect("field must have an identifier").to_string();
            let is_partition_key = args.partition_keys().contains(&field_name);
            let is_clustering_key = args.clustering_keys().contains(&field_name);
            let is_static_column = args.static_columns().contains(&field_name);

            let ch_field = Field::from_field(field, is_partition_key, is_clustering_key, is_static_column);

            if ch_field.is_tuple() && !ch_field.is_frozen() {
                panic!("Tuple field {} must be frozen. Use Frozen<Tuple<T>>.", field_name);
            }

            if is_partition_key && is_clustering_key {
                panic!("Field {} cannot be both partition and clustering key", field_name);
            }

            if is_static_column && (is_partition_key || is_clustering_key) {
                panic!(
                    "Field {} cannot be both static column and partition or clustering key",
                    field_name
                );
            }

            me.all_fields.push(ch_field);
        }

        me
    }

    pub fn populate(&'a mut self, args: &CharybdisMacroArgs) -> &'a Self {
        let mut partition_key_fields: Vec<Option<&Field>> = vec![None; args.partition_keys().len()];
        let mut clustering_key_fields: Vec<Option<&Field>> = vec![None; args.clustering_keys().len()];
        let mut primary_key_fields: Vec<Option<&Field>> =
            vec![None; args.partition_keys().len() + args.clustering_keys().len()];

        // primary key indexes by name
        let partition_key_indexes_by_name = args
            .partition_keys()
            .iter()
            .enumerate()
            .map(|(i, key)| (key, i))
            .collect::<HashMap<&String, usize>>();

        let clustering_key_indexes_by_name = args
            .clustering_keys()
            .iter()
            .enumerate()
            .map(|(i, key)| (key, i))
            .collect::<HashMap<&String, usize>>();

        #[allow(suspicious_double_ref_op)]
        let primary_key_indexes_by_name = args
            .primary_key()
            .iter()
            .enumerate()
            .map(|(i, key)| (key.clone(), i))
            .collect::<HashMap<&String, usize>>();

        // used to validate that provided macro args are present in struct fields
        let mut pk_struct_fields = HashSet::new();
        let mut ck_struct_fields = HashSet::new();
        let mut static_struct_fields = HashSet::new();

        // populate fields
        for ch_field in self.all_fields.iter() {
            if !ch_field.ignore {
                self.db_fields.push(ch_field);
            }

            if ch_field.is_static_column {
                static_struct_fields.insert(ch_field.name.clone());
            }

            if args.global_secondary_indexes().contains(&ch_field.name) {
                self.global_secondary_index_fields.push(ch_field);
            }

            if args.local_secondary_indexes().contains(&ch_field.name) {
                self.local_secondary_index_fields.push(ch_field);
            }

            if ch_field.is_partition_key {
                let partition_key_index = *partition_key_indexes_by_name
                    .get(&ch_field.name)
                    .expect("index must be set");
                partition_key_fields[partition_key_index] = Some(ch_field);

                let primary_key_index = *primary_key_indexes_by_name
                    .get(&ch_field.name)
                    .expect("index must be set");
                primary_key_fields[primary_key_index] = Some(ch_field);

                pk_struct_fields.insert(ch_field.name.clone());
            } else if ch_field.is_clustering_key {
                let clustering_key_index = *clustering_key_indexes_by_name
                    .get(&ch_field.name)
                    .expect("index must be set");
                clustering_key_fields[clustering_key_index] = Some(ch_field);

                let primary_key_index = *primary_key_indexes_by_name
                    .get(&ch_field.name)
                    .expect("index must be set");
                primary_key_fields[primary_key_index] = Some(ch_field);

                ck_struct_fields.insert(ch_field.name.clone());
            }
        }

        // validate that provided macro args are present in struct fields
        for key in args.partition_keys() {
            if !pk_struct_fields.contains(key) {
                panic!("Partition key {} not found in struct fields", key);
            }
        }

        for key in args.clustering_keys() {
            if !ck_struct_fields.contains(key) {
                panic!("Clustering key {} not found in struct fields", key);
            }
        }

        for key in args.static_columns() {
            if !static_struct_fields.contains(key) {
                panic!("Static column {} not found in struct fields", key);
            }
        }

        // populate primary key fields
        self.partition_key_fields = partition_key_fields.into_iter().flatten().collect();
        self.clustering_key_fields = clustering_key_fields.into_iter().flatten().collect();
        self.primary_key_fields = primary_key_fields.into_iter().flatten().collect();

        self
    }

    pub fn non_primary_key_db_fields(&self) -> Vec<&Field<'a>> {
        self.db_fields
            .iter()
            .filter(|field| !field.is_primary_key())
            .cloned()
            .collect()
    }

    pub fn non_db_fields(&'a self) -> Vec<&'a Field<'a>> {
        self.all_fields.iter().filter(|field| field.ignore).collect()
    }

    pub(crate) fn db_fields(named_fields: &'a FieldsNamed) -> Vec<Field<'a>> {
        let mut db_fields = vec![];

        for field in &named_fields.named {
            let field = Field::from_field(field, false, false, false);

            if !field.ignore {
                db_fields.push(field);
            }
        }

        db_fields
    }

    pub fn from_input(input: &'a DeriveInput, args: &'a CharybdisMacroArgs) -> Self {
        match &input.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(named_fields) => CharybdisFields::new(named_fields, args),
                _ => panic!("#[charybdis_model] works only for structs with named fields!"),
            },
            _ => panic!("#[charybdis_model] works only on structs!"),
        }
    }

    /// Map charybdis(ignore) to scylla(skip)
    pub fn proxy_charybdis_attrs_to_scylla(input: &mut DeriveInput) {
        if let Data::Struct(data_struct) = &mut input.data
            && let Fields::Named(fields_named) = &mut data_struct.fields
        {
            for field in &mut fields_named.named {
                if let Some(ignore) = &FieldAttributes::from_attributes(&field.attrs).unwrap().ignore
                    && *ignore
                {
                    field.attrs.push(syn::parse_quote!(#[scylla(skip)]));
                }
            }
        }
    }

    pub fn strip_charybdis_attributes(input: &mut DeriveInput) {
        if let Data::Struct(data_struct) = &mut input.data
            && let Fields::Named(fields_named) = &mut data_struct.fields
        {
            for field in &mut fields_named.named {
                field.attrs.retain(|attr| !attr.path().is_ident("charybdis"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    const USERS: &str = "table_name = users, partition_keys = [id], clustering_keys = []";

    /// Builds the fields of `input` the way the model macros do and hands them to `check`.
    fn with_fields(args: &str, input: &str, check: impl FnOnce(&CharybdisFields)) {
        let args: CharybdisMacroArgs = syn::parse_str(args).expect("valid macro arguments");
        let input: DeriveInput = syn::parse_str(input).expect("valid item");
        let mut fields = CharybdisFields::from_input(&input, &args);

        check(fields.populate(&args));
    }

    fn names(fields: &[&Field]) -> Vec<String> {
        fields.iter().map(|field| field.name.clone()).collect()
    }

    fn field<'a>(fields: &'a CharybdisFields, name: &str) -> &'a Field<'a> {
        fields
            .all_fields
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("no field named {}", name))
    }

    #[test]
    fn keys_follow_the_order_of_the_macro_arguments() {
        with_fields(
            "table_name = events, partition_keys = [tenant_id, bucket], clustering_keys = [created_at, id]",
            // declared in a different order than the key
            "struct Event { id: Uuid, name: Text, created_at: Timestamp, bucket: Int, tenant_id: Uuid }",
            |fields| {
                assert_eq!(names(&fields.partition_key_fields), ["tenant_id", "bucket"]);
                assert_eq!(names(&fields.clustering_key_fields), ["created_at", "id"]);
                assert_eq!(
                    names(&fields.primary_key_fields),
                    ["tenant_id", "bucket", "created_at", "id"]
                );
                // every other list keeps the order of the struct
                assert_eq!(
                    names(&fields.db_fields),
                    ["id", "name", "created_at", "bucket", "tenant_id"]
                );
                assert_eq!(names(&fields.non_primary_key_db_fields()), ["name"]);
            },
        );
    }

    #[test]
    fn index_fields_are_collected() {
        with_fields(
            "table_name = users, partition_keys = [id], clustering_keys = [created_at], \
             global_secondary_indexes = [email], local_secondary_indexes = [status]",
            "struct User { id: Uuid, created_at: Timestamp, email: Text, status: Text }",
            |fields| {
                assert_eq!(names(&fields.global_secondary_index_fields), ["email"]);
                assert_eq!(names(&fields.local_secondary_index_fields), ["status"]);
            },
        );
    }

    #[test]
    fn static_columns_are_marked() {
        with_fields(
            "table_name = users, partition_keys = [id], clustering_keys = [created_at], static_columns = [tenant]",
            "struct User { id: Uuid, created_at: Timestamp, tenant: Text, name: Text }",
            |fields| {
                assert!(field(fields, "tenant").is_static_column);
                assert!(!field(fields, "name").is_static_column);
            },
        );
    }

    #[test]
    fn ignored_fields_are_not_database_fields() {
        with_fields(
            USERS,
            "struct User { id: Uuid, name: Text, #[charybdis(ignore)] cache: Option<Session> }",
            |fields| {
                assert_eq!(names(&fields.db_fields), ["id", "name"]);
                assert_eq!(names(&fields.non_db_fields()), ["cache"]);
                assert!(field(fields, "cache").ignore);
                assert!(field(fields, "cache").outer_type == CqlType::Ignored);
            },
        );
    }

    #[test]
    fn column_type_overrides_the_type_of_the_field() {
        with_fields(
            USERS,
            r#"struct User { id: Uuid, #[charybdis(column_type = "Text")] settings: Settings, other: Settings }"#,
            |fields| {
                assert!(field(fields, "settings").outer_type == CqlType::Text);
                assert_eq!(field(fields, "settings").column_type_override.as_deref(), Some("Text"));

                // without the attribute a type that is not a CQL type is not recognised
                assert!(field(fields, "other").outer_type == CqlType::Unknown);
                assert_eq!(field(fields, "other").column_type_override, None);
            },
        );
    }

    #[test]
    fn option_does_not_hide_the_type() {
        with_fields(
            USERS,
            "struct User { id: Uuid, bio: Option<Text>, tags: Option<Set<Text>>, likes: Option<Counter> }",
            |fields| {
                assert!(field(fields, "bio").outer_type == CqlType::Text);
                assert!(field(fields, "tags").is_set());
                assert!(field(fields, "likes").is_counter());
            },
        );
    }

    #[test]
    fn the_last_path_segment_names_the_type() {
        with_fields(
            USERS,
            "struct User { id: charybdis::types::Uuid, name: charybdis::types::Text }",
            |fields| {
                assert!(field(fields, "id").outer_type == CqlType::Uuid);
                assert!(field(fields, "name").outer_type == CqlType::Text);
            },
        );
    }

    #[test]
    fn collections_are_recognised_unless_frozen() {
        with_fields(
            USERS,
            "struct User { id: Uuid, list: List<Int>, set: Set<Int>, map: Map<Text, Int>, \
             frozen: Frozen<List<Int>>, pair: Frozen<Tuple<Int, Text>>, likes: Counter }",
            |fields| {
                assert!(field(fields, "list").is_list() && field(fields, "list").is_collection());
                assert!(field(fields, "set").is_set() && field(fields, "set").is_collection());
                assert!(field(fields, "map").is_map() && field(fields, "map").is_collection());

                // a frozen collection can only be replaced as a whole, so it gets no push/pull helpers
                assert!(field(fields, "frozen").is_frozen());
                assert!(!field(fields, "frozen").is_collection());
                assert!(field(fields, "pair").is_frozen());
                assert!(!field(fields, "pair").is_collection());

                assert!(field(fields, "likes").is_counter());
                assert!(!field(fields, "likes").is_collection());
                assert!(!field(fields, "id").is_frozen());
            },
        );
    }

    #[test]
    #[should_panic(expected = "Tuple field pair must be frozen")]
    fn tuple_has_to_be_frozen() {
        with_fields(USERS, "struct User { id: Uuid, pair: Tuple<Int, Text> }", |_| {});
    }

    #[test]
    #[should_panic(expected = "Field id cannot be both partition and clustering key")]
    fn field_cannot_be_partition_and_clustering_key() {
        with_fields(
            "table_name = users, partition_keys = [id], clustering_keys = [id]",
            "struct User { id: Uuid }",
            |_| {},
        );
    }

    #[test]
    #[should_panic(expected = "Field id cannot be both static column and partition or clustering key")]
    fn key_cannot_be_a_static_column() {
        with_fields(
            "table_name = users, partition_keys = [id], clustering_keys = [], static_columns = [id]",
            "struct User { id: Uuid }",
            |_| {},
        );
    }

    #[test]
    #[should_panic(expected = "Partition key tenant_id not found in struct fields")]
    fn partition_key_has_to_be_a_field() {
        with_fields(
            "table_name = users, partition_keys = [tenant_id], clustering_keys = []",
            "struct User { id: Uuid }",
            |_| {},
        );
    }

    #[test]
    #[should_panic(expected = "Clustering key created_at not found in struct fields")]
    fn clustering_key_has_to_be_a_field() {
        with_fields(
            "table_name = users, partition_keys = [id], clustering_keys = [created_at]",
            "struct User { id: Uuid }",
            |_| {},
        );
    }

    #[test]
    #[should_panic(expected = "Static column tenant not found in struct fields")]
    fn static_column_has_to_be_a_field() {
        with_fields(
            "table_name = users, partition_keys = [id], clustering_keys = [], static_columns = [tenant]",
            "struct User { id: Uuid }",
            |_| {},
        );
    }

    #[test]
    #[should_panic(expected = "#[charybdis_model] works only on structs!")]
    fn enums_are_rejected() {
        with_fields(USERS, "enum User { Admin, Guest }", |_| {});
    }

    #[test]
    #[should_panic(expected = "#[charybdis_model] works only for structs with named fields!")]
    fn tuple_structs_are_rejected() {
        with_fields(USERS, "struct User(Uuid);", |_| {});
    }

    #[test]
    fn charybdis_attributes_are_translated_for_the_driver_and_removed() {
        let mut input: DeriveInput = syn::parse_str(
            r#"
                struct User {
                    id: Uuid,
                    #[charybdis(ignore)]
                    #[serde(skip)]
                    cache: Option<Session>,
                    #[charybdis(column_type = "Text")]
                    settings: Settings,
                }
            "#,
        )
        .expect("valid struct");

        CharybdisFields::proxy_charybdis_attrs_to_scylla(&mut input);
        CharybdisFields::strip_charybdis_attributes(&mut input);

        // an ignored field is skipped by the driver; attributes of other crates are left alone
        let expected: DeriveInput = syn::parse_str(
            r#"
                struct User {
                    id: Uuid,
                    #[serde(skip)]
                    #[scylla(skip)]
                    cache: Option<Session>,
                    settings: Settings,
                }
            "#,
        )
        .expect("valid struct");

        assert_eq!(
            input.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
    }
}

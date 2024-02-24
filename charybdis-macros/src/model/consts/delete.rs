use crate::utils::where_placeholders;
use charybdis_parser::fields::CharybdisFields;
use charybdis_parser::macro_args::CharybdisMacroArgs;
use quote::quote;
use syn::ImplItem;

pub(crate) fn delete_query_const(ch_args: &CharybdisMacroArgs, fields: &CharybdisFields) -> ImplItem {
    let query_str: String = format!(
        "DELETE FROM {} WHERE {}",
        ch_args.table_name(),
        where_placeholders(&fields.primary_key_fields),
    );

    let generated = quote! {
        const DELETE_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#generated)
}

pub(crate) fn delete_by_partition_key_query_const(ch_args: &CharybdisMacroArgs, fields: &CharybdisFields) -> ImplItem {
    let query_str: String = format!(
        "DELETE FROM {} WHERE {}",
        ch_args.table_name(),
        where_placeholders(&fields.partition_key_fields),
    );

    let generated = quote! {
        const DELETE_BY_PARTITION_KEY_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#generated)
}

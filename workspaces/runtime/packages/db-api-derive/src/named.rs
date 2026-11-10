use super::*;

pub fn expand(input: &DeriveInput) -> syn::Result<Tokens> {
    let name = &input.ident;
    if input.generics.type_params().next().is_some()
        || input.generics.const_params().next().is_some()
    {
        return Err(syn::Error::new_spanned(
            input,
            "DbQuery requires concrete owned arguments",
        ));
    }
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let mut database = None::<Path>;
    let mut mode = None::<Ident>;
    let mut cardinality = None::<Ident>;
    let mut result = None::<Type>;
    let mut scalar = None::<Path>;
    let mut projection = None::<Path>;
    let mut row_owner = None::<Path>;
    let mut sql = None::<LitStr>;
    let mut refs = Vec::<(Ident, Path, bool)>::new();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("db_query")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("database") {
                database = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("mode") {
                mode = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("cardinality") {
                cardinality = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("result") {
                result = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("decode") {
                scalar = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("projection") {
                projection = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("row") {
                row_owner = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("sql") {
                sql = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("tables") || meta.path.is_ident("columns") {
                let table = meta.path.is_ident("tables");
                meta.parse_nested_meta(|reference| {
                    let ident = reference
                        .path
                        .get_ident()
                        .cloned()
                        .ok_or_else(|| reference.error("expected reference name"))?;
                    if refs.iter().any(|(old, _, _)| *old == ident) {
                        return Err(reference.error("duplicate reference"));
                    }
                    refs.push((ident, reference.value()?.parse()?, table));
                    Ok(())
                })?;
            } else {
                return Err(meta.error("unknown DbQuery option"));
            }
            Ok(())
        })?;
    }
    let required = |what: &str| syn::Error::new_spanned(input, format!("missing {what}"));
    let database = database.ok_or_else(|| required("database"))?;
    let result = result.ok_or_else(|| required("result"))?;
    let mode = mode.ok_or_else(|| required("mode"))?;
    let mode_type = match mode.to_string().as_str() {
        "read" => quote!(::studio_db_api::ReadMode),
        "write" => quote!(::studio_db_api::WriteMode),
        _ => return Err(syn::Error::new_spanned(mode, "expected read or write")),
    };
    let read_only = mode == "read";
    let card = cardinality.ok_or_else(|| required("cardinality"))?;
    let card_type = match card.to_string().as_str() {
        "one" => quote!(::studio_db_api::One),
        "optional" => quote!(::studio_db_api::Optional),
        "many" => quote!(::studio_db_api::Many),
        "none" => quote!(::studio_db_api::NoRows),
        _ => {
            return Err(syn::Error::new_spanned(
                card,
                "expected one, optional, many or none",
            ));
        }
    };
    let sql = sql.ok_or_else(|| required("sql"))?;
    let text = sql.value();
    // SQLite owns statement, parameter and access validation.
    let mut fragments = Vec::new();
    let mut remaining = text.as_str();
    let mut used = vec![false; refs.len()];
    while let Some(start) = remaining.find('{') {
        let literal = &remaining[..start];
        fragments.push(quote!(#literal));
        let end = remaining[start + 1..]
            .find('}')
            .ok_or_else(|| syn::Error::new_spanned(&sql, "unclosed descriptor reference"))?
            + start
            + 1;
        let key = &remaining[start + 1..end];
        let i = refs
            .iter()
            .position(|(name, _, _)| name == key)
            .ok_or_else(|| {
                syn::Error::new_spanned(&sql, format!("unknown descriptor reference {key}"))
            })?;
        used[i] = true;
        let (_, path, table) = &refs[i];
        fragments.push(if *table {
            quote!("\"", ::studio_db_api::query_table::<#database,#path>(), "\"")
        } else {
            quote!(::studio_db_api::query_column::<#database,_>(#path))
        });
        remaining = &remaining[end + 1..];
    }
    if remaining.contains('}') || used.contains(&false) {
        return Err(syn::Error::new_spanned(
            &sql,
            "unused or malformed descriptor reference",
        ));
    }
    fragments.push(quote!(#remaining));
    let syn::Data::Struct(data) = &input.data else {
        return Err(required("argument struct"));
    };
    let mut binds = Vec::new();
    let mut values = Vec::new();
    for (i, field) in data.fields.iter().enumerate() {
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(field, "named arguments required"))?;
        let mut column = None::<Path>;
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("db")) {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("column") {
                    return Err(meta.error("expected column = Row::COLUMN"));
                }
                column = Some(meta.value()?.parse()?);
                Ok(())
            })?;
        }
        let column = column.ok_or_else(|| {
            syn::Error::new_spanned(
                field,
                "each query argument requires an explicit column codec",
            )
        })?;
        let variable = format_ident!("encoded_{i}");
        let value = if matches!(field.ty, Type::Reference(_)) {
            quote!(self.#ident)
        } else {
            quote!(&self.#ident)
        };
        binds.push(quote!(let _ = ::studio_db_api::query_column::<#database,_>(#column); let #variable = ::studio_db_api::ColumnInput::encode_input(#value,connection,#column)?;));
        values.push(quote!(&#variable as &dyn ::studio_db_api::rusqlite::types::ToSql));
    }
    let count = values.len();
    let (decoder, columns) = match (scalar, projection, row_owner) {
        (Some(column), None, None) => (
            quote!({ let _ = ::studio_db_api::query_column::<#database,_>(#column); ::studio_db_api::projection_value(connection,row,0,#column) }),
            quote!(&[::studio_db_api::query_column_descriptor(#column)]),
        ),
        (None, Some(owner), None) => (
            quote!({ let _ = ::studio_db_api::query_table::<#database,<#owner as ::studio_db_api::Projection>::Table>(); <#owner as ::studio_db_api::Projection>::read(connection,row) }),
            quote!(<#owner as ::studio_db_api::Projection>::COLUMNS),
        ),
        (None, None, Some(owner)) => (
            quote!({ let _ = ::studio_db_api::query_table::<#database,#owner>(); <#owner as ::studio_db_api::Table>::read(connection,row) }),
            quote!(<#owner as ::studio_db_api::Table>::COLUMNS),
        ),
        (None, None, None) if card == "none" => (quote!(Ok(())), quote!(&[])),
        _ => return Err(required("exactly one decode, projection or row codec")),
    };
    let tables = refs
        .iter()
        .filter(|(_, _, table)| *table)
        .map(|(_, path, _)| quote!(::studio_db_api::query_table::<#database,#path>()));
    let declaration_constructor = format_ident!("__generated_{}", "declare");
    // Keep declaration validation beside the generated token for module enumeration.
    Ok(quote! {
        impl #impl_generics ::studio_db_api::QueryParams for #name #type_generics #where_clause {
            const COUNT: usize = #count;
            fn bind<T>(&self, connection: &::studio_db_api::rusqlite::Connection, f: impl FnOnce(&[&dyn ::studio_db_api::rusqlite::types::ToSql]) -> ::studio_db_api::rusqlite::Result<T>) -> ::studio_db_api::rusqlite::Result<T> { let _=connection; #(#binds)* f(&[#(#values),*]) }
        }
        impl #impl_generics #name #type_generics #where_clause {
            pub const QUERY: ::studio_db_api::NamedQuery<#database,Self,#result,#mode_type,#card_type> = {
                fn decode(connection: &::studio_db_api::rusqlite::Connection,row: &::studio_db_api::rusqlite::Row<'_>) -> ::studio_db_api::rusqlite::Result<#result> { let _=(connection,row); #decoder }
                ::studio_db_api::NamedQuery::#declaration_constructor(::studio_db_api::static_sql!(#(#fragments),*),decode,#columns,&[#(#tables),*])
            };
            pub fn validate_query(connection: &::studio_db_api::rusqlite::Connection) -> ::std::result::Result<(),::studio_db_api::QueryValidationError> { Self::QUERY.validate(connection,#read_only) }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_query_placeholders_require_declared_refs() {
        for sql in ["SELECT {missing}", "SELECT {unclosed", "SELECT }extra"] {
            let input = syn::parse_quote! { #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::ID,sql=#sql)] struct Query; };
            assert!(expand(&input).is_err(), "{sql}");
        }
        let input = syn::parse_quote! { #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::ID,columns(unused=Row::ID),sql="SELECT 1")] struct Query; };
        assert!(expand(&input).is_err());
    }
}

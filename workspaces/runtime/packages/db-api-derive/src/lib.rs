//! Generates ordinary positional rusqlite code; no compile-time database access.
use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{
    DeriveInput, Expr, Fields, Ident, LitStr, Path, Type, parse_macro_input, spanned::Spanned,
};

#[proc_macro_derive(DbTable, attributes(db))]
pub fn table(input: TokenStream) -> TokenStream {
    expand_table(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro_derive(DbPartition, attributes(db))]
pub fn partition(input: TokenStream) -> TokenStream {
    expand_table(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro_derive(DbEnum)]
pub fn enumeration(input: TokenStream) -> TokenStream {
    expand_enum(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
// SQLite's keyword set. One owner for all unquoted generated identifiers.
const SQL_KEYWORDS: &str = "abort action add after all alter always analyze and as asc attach autoincrement before begin between by cascade case cast check collate column commit conflict constraint create cross current current_date current_time current_timestamp database default deferrable deferred delete desc detach distinct do drop each else end escape except exclude exclusive exists explain fail filter first following for foreign from full generated glob group groups having if ignore immediate in index indexed initially inner insert instead intersect into is isnull join key last left like limit match materialized natural no not nothing notnull null nulls of offset on or order others outer over partition plan pragma preceding primary query raise range recursive references regexp reindex release rename replace restrict returning right rollback row rows savepoint select set table temp temporary then ties to transaction trigger unbounded union unique update using vacuum values view virtual when where window with without";
fn identifier(value: &str, span: &impl Spanned) -> syn::Result<()> {
    identifier_shape(value, span)?;
    if SQL_KEYWORDS
        .split_ascii_whitespace()
        .any(|keyword| keyword == value)
    {
        return Err(syn::Error::new(
            span.span(),
            "SQL identifier must not be a SQLite keyword",
        ));
    }
    Ok(())
}
fn identifier_shape(value: &str, span: &impl Spanned) -> syn::Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .enumerate()
            .all(|(i, c)| c == b'_' || c.is_ascii_lowercase() || (i > 0 && c.is_ascii_digit()))
    {
        return Err(syn::Error::new(
            span.span(),
            "SQL identifier must match [a-z_][a-z0-9_]* and must not be a SQLite keyword",
        ));
    }
    Ok(())
}
fn quoted(value: &str) -> String {
    format!("\"{value}\"")
}
fn expand_enum(input: &DeriveInput) -> syn::Result<Tokens> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "DbEnum does not accept generics",
        ));
    }
    let syn::Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "DbEnum requires an enum"));
    };
    let mut variants = Vec::new();
    let mut values = Vec::new();
    let mut constants = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "DbEnum requires closed fieldless variants",
            ));
        }
        let Some((_, code)) = &variant.discriminant else {
            return Err(syn::Error::new_spanned(
                variant,
                "each variant needs an explicit permanent integer discriminant",
            ));
        };
        let value = integer(code)?;
        variants.push(&variant.ident);
        values.push(value);
        let constant = format_ident!("DB_CODE_{}", variant.ident.to_string().to_uppercase());
        let literal = value.to_string();
        constants.push(quote!(pub const #constant: &'static str = #literal;));
    }
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(name, "DbEnum cannot be empty"));
    }
    let codes = values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    Ok(quote! {
        impl #name { #(#constants)* }
        impl ::studio_db_api::EnumValue for #name {}
        impl ::studio_db_api::DbValue for #name {
            const SQL_TYPE: &'static str = "INTEGER";
            const CODES: &'static str = #codes;
            const CODE_NAMES: &'static [(i64, &'static str)] = &[#((#values, stringify!(#variants))),*];
        }
        impl ::studio_db_api::rusqlite::types::ToSql for #name {
            fn to_sql(&self) -> ::studio_db_api::rusqlite::Result<::studio_db_api::rusqlite::types::ToSqlOutput<'_>> {
                let code: i64 = match self { #(Self::#variants => #values),* };
                Ok(::studio_db_api::rusqlite::types::ToSqlOutput::Owned(code.into()))
            }
        }
        impl ::studio_db_api::rusqlite::types::FromSql for #name {
            fn column_result(value: ::studio_db_api::rusqlite::types::ValueRef<'_>) -> ::studio_db_api::rusqlite::types::FromSqlResult<Self> {
                match value.as_i64()? { #(#values => Ok(Self::#variants),)* code => Err(::studio_db_api::rusqlite::types::FromSqlError::OutOfRange(code)) }
            }
        }
    })
}
fn integer(expr: &Expr) -> syn::Result<i64> {
    if let Expr::Group(group) = expr {
        return integer(&group.expr);
    }
    if let Expr::Paren(group) = expr {
        return integer(&group.expr);
    }
    if let Expr::Lit(value) = expr
        && let syn::Lit::Int(value) = &value.lit
    {
        return value.base10_parse();
    }
    if let Expr::Unary(value) = expr
        && matches!(value.op, syn::UnOp::Neg(_))
    {
        return integer(&value.expr)?
            .checked_neg()
            .ok_or_else(|| syn::Error::new_spanned(expr, "code is outside i64"));
    }
    Err(syn::Error::new_spanned(
        expr,
        "use an explicit i64 integer literal for the permanent code",
    ))
}
#[derive(Default)]
struct Index {
    name: Option<LitStr>,
    columns: Vec<Ident>,
    unique: bool,
    predicate: Option<Expr>,
}
struct Field {
    name: Ident,
    ty: Type,
    column: String,
    key: bool,
    private: bool,
    codec: Option<Path>,
    check: Option<LitStr>,
    fk: Option<Path>,
    generated: bool,
}
fn expand_table(input: &DeriveInput) -> syn::Result<Tokens> {
    let name = &input.ident;
    let input_vis = &input.vis;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "DbTable requires a concrete row type",
        ));
    }
    let mut partition_owner = None::<Path>;
    let mut partition_kind = None::<Path>;
    let mut partitioned_by = None::<Path>;
    let mut table = None::<LitStr>;
    let mut database = None::<Path>;
    let mut check = None::<Path>;
    let mut state_check = None::<Path>;
    let (mut strict, mut without_rowid, mut receipt) = (false, false, false);
    let mut indexes = Vec::new();
    let mut legacy_ddl = None::<LitStr>;
    let mut legacy_indexes = Vec::<LitStr>::new();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("db")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("of") {
                partition_owner = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("kind") {
                partition_kind = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("partitioned_by") {
                partitioned_by = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("table") {
                table = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("database") {
                database = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("check") {
                check = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("state_check") {
                state_check = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("strict") {
                strict = true;
            } else if meta.path.is_ident("without_rowid") {
                without_rowid = true;
            } else if meta.path.is_ident("receipt") {
                receipt = true;
            } else if meta.path.is_ident("legacy_ddl") {
                legacy_ddl = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("legacy_index") {
                legacy_indexes.push(meta.value()?.parse()?);
            } else if meta.path.is_ident("index") {
                let mut index = Index::default();
                meta.parse_nested_meta(|part| {
                    if part.path.is_ident("name") {
                        index.name = Some(part.value()?.parse()?);
                    } else if part.path.is_ident("unique") {
                        index.unique = true;
                    } else if part.path.is_ident("predicate") {
                        index.predicate = Some(part.value()?.parse()?);
                    } else if part.path.is_ident("columns") {
                        part.parse_nested_meta(|col| {
                            index.columns.push(col.path.require_ident()?.clone());
                            Ok(())
                        })?;
                    } else {
                        return Err(part.error("unknown index option"));
                    }
                    Ok(())
                })?;
                indexes.push(index);
            } else {
                return Err(meta.error("unknown table option"));
            }
            Ok(())
        })?;
    }
    if partition_owner.is_some() && !legacy_indexes.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "partition indexes require scoped index declarations",
        ));
    }
    if partition_owner.is_some() {
        if table.is_some() || database.is_some() || legacy_ddl.is_some() || partitioned_by.is_some()
        {
            return Err(syn::Error::new_spanned(
                name,
                "partition inherits its physical table and database",
            ));
        }
        table = Some(syn::parse_quote!("__db_partition_table"));
        database = Some(syn::parse_quote!(__PartitionDatabase));
    } else if partition_kind.is_some() {
        return Err(syn::Error::new_spanned(
            name,
            "kind requires of = PhysicalTable",
        ));
    }
    let partition_predicate = if let Some(owner) = &partition_owner {
        let kind = partition_kind
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(name, "missing kind = Kind::Variant"))?;
        let mut enum_owner = kind.clone();
        let variant = enum_owner
            .segments
            .pop()
            .expect("kind variant")
            .into_value()
            .ident;
        enum_owner.segments.pop_punct();
        let code = format_ident!("DB_CODE_{}", variant.to_string().to_uppercase());
        quote!(
            ::studio_db_api::static_sql!("\"", <#owner as ::studio_db_api::PartitionedTable>::KIND_COLUMN, "\"=", #enum_owner::#code)
        )
    } else {
        quote!("")
    };
    let sql = |text: &str| -> Tokens {
        if let Some(owner) = &partition_owner {
            let parts = text.split("__db_partition_table").collect::<Vec<_>>();
            let mut pieces = Vec::new();
            for (i, part) in parts.iter().enumerate() {
                if i > 0 {
                    pieces.push(quote!(<#owner as ::studio_db_api::Table>::NAME));
                }
                pieces.push(quote!(#part));
            }
            quote!(::studio_db_api::static_sql!(#(#pieces),*))
        } else {
            quote!(#text)
        }
    };
    let table =
        table.ok_or_else(|| syn::Error::new_spanned(name, "missing #[db(table = \"name\")]"))?;
    if legacy_ddl.is_some() {
        identifier_shape(&table.value(), &table)?;
    } else {
        identifier(&table.value(), &table)?;
    }
    let database = database
        .ok_or_else(|| syn::Error::new_spanned(name, "missing #[db(database = Marker)]"))?;
    let database = partition_owner.as_ref().map_or_else(
        || quote!(#database),
        |owner| quote!(<#owner as ::studio_db_api::Table>::Database),
    );
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "DbTable requires a struct"));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "DbTable requires named fields",
        ));
    };
    let mut fields = Vec::new();
    let mut revision_field = None::<Ident>;
    for field in &named.named {
        let field_name = field.ident.clone().expect("named field");
        let mut f = Field {
            column: field_name.to_string(),
            name: field_name,
            ty: field.ty.clone(),
            key: false,
            private: false,
            codec: None,
            check: None,
            fk: None,
            generated: false,
        };
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("db")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("revision") {
                    if revision_field.replace(f.name.clone()).is_some() {
                        return Err(meta.error("only one revision field is allowed"));
                    }
                } else if meta.path.is_ident("key") {
                    f.key = true;
                } else if meta.path.is_ident("generated") {
                    f.generated = true;
                } else if meta.path.is_ident("private") {
                    f.private = true;
                } else if meta.path.is_ident("column") {
                    f.column = meta.value()?.parse::<LitStr>()?.value();
                } else if meta.path.is_ident("codec") {
                    f.codec = Some(meta.value()?.parse()?);
                } else if meta.path.is_ident("check") {
                    f.check = Some(meta.value()?.parse()?);
                } else if meta.path.is_ident("fk") {
                    f.fk = Some(meta.value()?.parse()?);
                } else {
                    return Err(meta.error("unknown field option"));
                }
                Ok(())
            })?;
        }
        if legacy_ddl.is_some() {
            identifier_shape(&f.column, field)?;
        } else {
            identifier(&f.column, field)?;
        }
        if fields
            .iter()
            .any(|previous: &Field| previous.column == f.column)
        {
            return Err(syn::Error::new_spanned(field, "duplicate SQL column"));
        }
        fields.push(f);
    }
    let keys = fields.iter().filter(|f| f.key).collect::<Vec<_>>();
    if keys.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "DbTable requires at least one #[db(key)] field",
        ));
    }
    if keys.iter().any(|f| f.codec.is_some()) {
        return Err(syn::Error::new_spanned(
            name,
            "v1 keys use native DbValue codecs",
        ));
    }
    let table_name = table.value();
    let column_list = fields
        .iter()
        .map(|f| quoted(&f.column))
        .collect::<Vec<_>>()
        .join(",");
    let where_key = keys
        .iter()
        .map(|f| format!("{}=?", quoted(&f.column)))
        .collect::<Vec<_>>()
        .join(" AND ");
    let key_columns = keys
        .iter()
        .map(|f| quoted(&f.column))
        .collect::<Vec<_>>()
        .join(",");
    let sql_table = quoted(&table_name);
    let select = format!("SELECT {column_list} FROM {sql_table}");
    let get = format!("{select} WHERE {where_key}");
    let writable = fields.iter().filter(|f| !f.generated).collect::<Vec<_>>();
    let write_columns = writable
        .iter()
        .map(|f| quoted(&f.column))
        .collect::<Vec<_>>()
        .join(",");
    let insert = if writable.is_empty() {
        format!("INSERT INTO {sql_table} DEFAULT VALUES")
    } else {
        format!(
            "INSERT INTO {sql_table}({write_columns}) VALUES({})",
            vec!["?"; writable.len()].join(",")
        )
    };
    let delete = format!("DELETE FROM {sql_table} WHERE {where_key}");
    let updates = fields
        .iter()
        .filter(|f| !f.key && !f.generated)
        .map(|f| format!("{}=excluded.{}", quoted(&f.column), quoted(&f.column)))
        .collect::<Vec<_>>();
    let generated_keys = keys.iter().filter(|f| f.generated).collect::<Vec<_>>();
    let update_columns = writable
        .iter()
        .enumerate()
        .filter(|(_, f)| !f.key)
        .map(|(i, f)| format!("{}=?{}", quoted(&f.column), i + 1))
        .collect::<Vec<_>>();
    let update_columns = if update_columns.is_empty() {
        format!("{}={}", quoted(&keys[0].column), quoted(&keys[0].column))
    } else {
        update_columns.join(",")
    };
    let update_keys = keys
        .iter()
        .map(|f| {
            let i = writable
                .iter()
                .position(|w| w.name == f.name)
                .unwrap_or_else(|| {
                    writable.len()
                        + generated_keys
                            .iter()
                            .position(|w| w.name == f.name)
                            .expect("generated key")
                });
            format!("{}=?{}", quoted(&f.column), i + 1)
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    let update_existing = format!("UPDATE {sql_table} SET {update_columns} WHERE {update_keys}");
    let upsert = if updates.is_empty() {
        format!("{insert} ON CONFLICT({key_columns}) DO NOTHING")
    } else {
        format!(
            "{insert} ON CONFLICT({key_columns}) DO UPDATE SET {}",
            updates.join(",")
        )
    };
    let mut ddl = vec![quote!(concat!("CREATE TABLE ", #table, "("))];
    let mut constraints = vec![quote!("1")];
    let mut reads = Vec::new();
    let mut binds = Vec::new();
    let mut encoded = Vec::new();
    let mut markers = Vec::new();
    let mut constants = Vec::new();
    let mut descriptors = Vec::new();
    for (i, f) in fields.iter().enumerate() {
        let Field {
            name: field_name,
            ty,
            column,
            codec,
            private,
            ..
        } = f;
        let stored = if let Some(codec) = codec {
            quote!(<#codec as ::studio_db_api::Codec<#ty>>::Stored)
        } else {
            quote!(#ty)
        };
        if f.key {
            let not_null = format!(" AND ({} IS NOT NULL)", quoted(column));
            constraints.push(quote!(#not_null));
        }
        if i > 0 {
            ddl.push(quote!(", "));
        }
        ddl.extend([
            quote!(#column),
            quote!(" "),
            quote!(<#stored as ::studio_db_api::DbValue>::SQL_TYPE),
        ]);
        if f.key && keys.len() == 1 {
            ddl.push(quote!(" PRIMARY KEY"));
            ddl.push(quote!(
                if matches!(<#stored as ::studio_db_api::DbValue>::SQL_TYPE.as_bytes(), b"INTEGER")
                {
                    ""
                } else {
                    " NOT NULL"
                }
            ));
        } else if f.key {
            ddl.push(quote!(" NOT NULL"));
        } else {
            ddl.push(quote!(if <#stored as ::studio_db_api::DbValue>::NULLABLE { "" } else { " NOT NULL" }));
        }
        if let Some(check) = &f.check {
            ddl.push(quote!(concat!(" CHECK(", #check, ")")));
            constraints.push(quote!(concat!(" AND (", #check, ")")));
        }
        ddl.extend([quote!(if <#stored as ::studio_db_api::DbValue>::CODES.is_empty() { "" } else { " CHECK(" }), quote!(if <#stored as ::studio_db_api::DbValue>::CODES.is_empty() { "" } else { #column }), quote!(if <#stored as ::studio_db_api::DbValue>::CODES.is_empty() { "" } else { " IN (" }), quote!(<#stored as ::studio_db_api::DbValue>::CODES), quote!(if <#stored as ::studio_db_api::DbValue>::CODES.is_empty() { "" } else { "))" })]);
        let quoted_column = quoted(column);
        constraints.extend([
            quote!(if <#stored as ::studio_db_api::DbValue>::CODES.is_empty() { "" } else { " AND (" }),
            quote!(if <#stored as ::studio_db_api::DbValue>::CODES.is_empty() { "" } else { #quoted_column }),
            quote!(if <#stored as ::studio_db_api::DbValue>::CODES.is_empty() { "" } else { " IN (" }),
            quote!(<#stored as ::studio_db_api::DbValue>::CODES),
            quote!(if <#stored as ::studio_db_api::DbValue>::CODES.is_empty() { "" } else { "))" }),
        ]);
        if let Some(fk) = &f.fk {
            let mut row = fk.clone();
            let target = row
                .segments
                .pop()
                .ok_or_else(|| syn::Error::new_spanned(fk, "expected Row::field"))?
                .into_value()
                .ident;
            if row.segments.is_empty() {
                return Err(syn::Error::new_spanned(fk, "expected Row::field"));
            }
            row.segments.pop_punct();
            let target_const = format_ident!("__DB_COLUMN_{}", target.to_string().to_uppercase());
            ddl.extend([
                quote!(" REFERENCES "),
                quote!(<#row as ::studio_db_api::Table>::NAME),
                quote!("("),
                quote!(#row::#target_const),
                quote!(")"),
            ]);
        }
        if let Some(codec) = codec {
            let var = format_ident!("encoded_{i}");
            reads.push(quote!(#field_name: <#codec as ::studio_db_api::Codec<#ty>>::decode(_connection, <#stored as ::studio_db_api::DbValue>::read(row, #i)?)?));
            if !f.generated {
                encoded.push(quote!(let #var = <#codec as ::studio_db_api::Codec<#ty>>::encode(_connection, &self.#field_name)?;));
                binds.push(quote!(&#var as &dyn ::studio_db_api::rusqlite::types::ToSql));
            }
        } else {
            reads.push(quote!(#field_name: <#ty as ::studio_db_api::DbValue>::read(row, #i)?));
            if !f.generated {
                binds.push(
                    quote!(&self.#field_name as &dyn ::studio_db_api::rusqlite::types::ToSql),
                );
            }
        }
        let marker = format_ident!(
            "{}Column{}",
            name,
            field_name
                .to_string()
                .split('_')
                .map(|s| {
                    let mut c = s.chars();
                    c.next()
                        .map(|first| first.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                })
                .collect::<String>()
        );
        let constant = format_ident!("{}", field_name.to_string().to_uppercase());
        let metadata = format_ident!("__DB_COLUMN_{}", field_name.to_string().to_uppercase());
        markers.push(quote!(#[derive(Clone, Copy)] #input_vis struct #marker;));
        if !f.generated {
            markers.push(quote!(impl ::studio_db_api::WritableColumn for #marker {}));
        }
        constants.push(quote!(pub const #metadata: &'static str = #column;));
        constants.push(quote!(pub const #constant: #marker = #marker;));
        let (parameter, encode) = if let Some(codec) = codec {
            (
                quote!(<#codec as ::studio_db_api::Codec<#ty>>::Encoded<'a>),
                quote!(<#codec as ::studio_db_api::Codec<#ty>>::encode(connection, value)),
            )
        } else {
            (
                quote!(&'a #ty),
                quote!({
                    let _ = connection;
                    Ok(value)
                }),
            )
        };
        markers.push(quote!(impl ::studio_db_api::Column for #marker {
            type Row = #name;
            type Value = #ty;
            type Stored = #stored;
            type Parameter<'a> = #parameter;
            const NAME: &'static str = #column;
            const QUOTED_NAME: &'static str = concat!("\"", #column, "\"");
            fn encode<'a>(connection: &::studio_db_api::rusqlite::Connection, value: &'a #ty) -> ::studio_db_api::rusqlite::Result<Self::Parameter<'a>> { #encode }
            fn decode(connection: &::studio_db_api::rusqlite::Connection, row: &::studio_db_api::rusqlite::Row<'_>, index: usize) -> ::studio_db_api::rusqlite::Result<Self::Value> { Self::__db_decode(connection, row, index) }
        }));
        let decode = if let Some(codec) = codec {
            quote!(<#codec as ::studio_db_api::Codec<#ty>>::decode(connection, <#stored as ::studio_db_api::DbValue>::read(row, index)?))
        } else {
            quote!(<#ty as ::studio_db_api::DbValue>::read(row, index))
        };
        markers.push(quote!(impl ::studio_db_api::ColumnInput<#marker> for #ty {
            type Parameter<'a> = <#marker as ::studio_db_api::Column>::Parameter<'a>;
            fn encode_input<'a>(&'a self, connection: &::studio_db_api::rusqlite::Connection, _: #marker) -> ::studio_db_api::rusqlite::Result<Self::Parameter<'a>> {
                <#marker as ::studio_db_api::Column>::encode(connection, self)
            }
        }));
        if codec.is_none() {
            let borrowed: Option<Type> = match quote!(#ty).to_string().replace(' ', "").as_str() {
                "String" => Some(syn::parse_quote!(str)),
                "Option<String>" => Some(syn::parse_quote!(Option<&str>)),
                _ => None,
            };
            if let Some(borrowed) = borrowed {
                markers.push(quote!(impl ::studio_db_api::ColumnInput<#marker> for #borrowed {
                    type Parameter<'a> = &'a Self where Self: 'a;
                    fn encode_input<'a>(&'a self, _: &::studio_db_api::rusqlite::Connection, _: #marker) -> ::studio_db_api::rusqlite::Result<Self::Parameter<'a>> { Ok(self) }
                }));
            }
        }
        markers.push(quote!(impl #marker {
            fn __db_decode(connection: &::studio_db_api::rusqlite::Connection, row: &::studio_db_api::rusqlite::Row<'_>, index: usize) -> ::studio_db_api::rusqlite::Result<#ty> { let _ = connection; #decode }
        }));
        if let Some(owner) = &partition_owner {
            markers.push(quote!(const _: () = {
                assert!(!::studio_db_api::identifiers_equal(#column, <#owner as ::studio_db_api::PartitionedTable>::KIND_COLUMN), "partition fields cannot expose the physical discriminator");
            };));
            markers.push(quote!(const _: fn() = || {
                fn same_storage<C: ::studio_db_api::Column<Stored = #stored>>(_: C) {}
                same_storage(#owner::#constant);
            };));
        }
        descriptors.push(quote!(::studio_db_api::ColumnDescriptor { name: #column, storage: <#stored as ::studio_db_api::DbValue>::SQL_TYPE, nullable: <#stored as ::studio_db_api::DbValue>::NULLABLE, private: #private, codes: <#stored as ::studio_db_api::DbValue>::CODE_NAMES }));
    }
    if keys.len() > 1 {
        let ddl_keys = keys
            .iter()
            .map(|f| f.column.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let pk = format!(", PRIMARY KEY({ddl_keys})");
        ddl.push(quote!(#pk));
    }
    let suffix = match (strict, without_rowid) {
        (true, true) => ") STRICT, WITHOUT ROWID",
        (true, false) => ") STRICT",
        (false, true) => ") WITHOUT ROWID",
        (false, false) => ")",
    };
    ddl.push(quote!(#suffix));
    if let Some(sql) = legacy_ddl {
        ddl = vec![quote!(#sql)];
    }
    let mut index_sql = Vec::new();
    for sql in legacy_indexes {
        let text = sql.value();
        let words = text.split_ascii_whitespace().collect::<Vec<_>>();
        let offset = if words.get(1) == Some(&"UNIQUE") {
            3
        } else {
            2
        };
        let index_name = words
            .get(offset)
            .ok_or_else(|| syn::Error::new_spanned(&sql, "expected CREATE INDEX name ON table"))?;
        let index_name = index_name.trim_matches('"');
        index_sql.push(quote!(::studio_db_api::IndexDescriptor { name: #index_name, sql: #sql }));
    }
    for index in indexes {
        let index_name = index
            .name
            .ok_or_else(|| syn::Error::new_spanned(name, "index requires a name"))?;
        identifier(&index_name.value(), &index_name)?;
        if index.columns.is_empty() {
            return Err(syn::Error::new_spanned(
                index_name,
                "index requires columns",
            ));
        }
        let columns = index
            .columns
            .iter()
            .map(|col| {
                fields
                    .iter()
                    .find(|f| f.name == *col)
                    .map(|f| f.column.as_str())
                    .ok_or_else(|| syn::Error::new_spanned(col, "unknown indexed field"))
            })
            .collect::<syn::Result<Vec<_>>>()?
            .join(",");
        let base = format!(
            "CREATE {}INDEX {} ON {table_name}({columns})",
            if index.unique { "UNIQUE " } else { "" },
            index_name.value()
        );
        let base = sql(&base);
        let predicate = index
            .predicate
            .map(|p| predicate_sql(&p, &fields))
            .transpose()?;
        let index_text = match (partition_owner.is_some(), predicate) {
            (true, Some(predicate)) => quote!(
                ::studio_db_api::static_sql!(#base, " WHERE ", #partition_predicate, " AND (", #predicate, ")")
            ),
            (true, None) => {
                quote!(::studio_db_api::static_sql!(#base, " WHERE ", #partition_predicate))
            }
            (false, Some(predicate)) => {
                quote!(::studio_db_api::static_sql!(#base, " WHERE ", #predicate))
            }
            (false, None) => base,
        };
        index_sql
            .push(quote!(::studio_db_api::IndexDescriptor { name: #index_name, sql: #index_text }));
    }
    let key_types = keys.iter().map(|f| &f.ty).collect::<Vec<_>>();
    let key_assertions = key_types.iter().map(|ty| quote!(assert!(!<#ty as ::studio_db_api::DbValue>::NULLABLE, "primary keys cannot be nullable");));
    let update = if check.is_none() && !receipt && !writable.is_empty() && partitioned_by.is_none()
    {
        quote!(impl ::studio_db_api::UpdateTable for #name {})
    } else {
        quote!()
    };
    let validate = check.map(|check| quote!(fn validate(&self) -> ::studio_db_api::rusqlite::Result<()> { #check(self).map_err(|error| ::studio_db_api::rusqlite::Error::ToSqlConversionFailure(Box::new(error))) }));
    let state_check = state_check.map(|check| quote!(fn state_check(connection: &::studio_db_api::rusqlite::Connection) -> ::studio_db_api::rusqlite::Result<::studio_db_api::StateCheck> { #check(connection) }));
    let insert_new = format!("{insert} ON CONFLICT({key_columns}) DO NOTHING");
    let key_binds = keys.iter().enumerate().map(|(i, _)| {
        let i = syn::Index::from(i);
        quote!(&key.#i as &dyn ::studio_db_api::rusqlite::types::ToSql)
    });
    let placeholders = vec!["?"; writable.len()].join(",");
    let mut partition_impl = quote!();
    if let Some(kind) = &partitioned_by {
        let field = fields
            .iter()
            .find(|f| {
                quote!(#kind).to_string() == {
                    let ty = &f.ty;
                    quote!(#ty).to_string()
                }
            })
            .ok_or_else(|| {
                syn::Error::new_spanned(kind, "partition kind requires a field of this enum type")
            })?;
        if !field.key {
            return Err(syn::Error::new_spanned(
                &field.name,
                "kind must be part of the primary key",
            ));
        }
        let column = &field.column;
        let partition_key_types = keys
            .iter()
            .filter(|f| f.name != field.name)
            .map(|f| &f.ty)
            .collect::<Vec<_>>();
        partition_impl = quote!(impl ::studio_db_api::PartitionedTable for #name { type Kind = #kind; const KIND_COLUMN: &'static str = #column; type PartitionKey = (#(#partition_key_types,)*); });
    }
    let table_name_expr = partition_owner.as_ref().map_or_else(
        || quote!(#table),
        |owner| quote!(<#owner as ::studio_db_api::Table>::NAME),
    );
    let select = sql(&select);
    let table_select = if partition_owner.is_some() {
        quote!(::studio_db_api::static_sql!(#select, " WHERE ", #partition_predicate))
    } else {
        select.clone()
    };
    let mut get = sql(&get);
    let mut insert = sql(&insert);
    let mut delete = sql(&delete);
    let mut upsert = sql(&upsert);
    let mut insert_new = sql(&insert_new);
    let mut update_existing = sql(&update_existing);
    let mut ddl = quote!(::studio_db_api::static_sql!(#(#ddl),*));

    let mut columns = quote!(&[#(#descriptors),*]);
    let constraints = quote!(::studio_db_api::static_sql!(#(#constraints),*));
    let mut state_prefix = quote!("");
    if let Some(owner) = &partition_owner {
        let kind = partition_kind.as_ref().expect("partition kind");
        let mut enum_owner = kind.clone();
        let variant = enum_owner
            .segments
            .pop()
            .expect("kind variant")
            .into_value()
            .ident;
        enum_owner.segments.pop_punct();
        let code = format_ident!("DB_CODE_{}", variant.to_string().to_uppercase());
        let typed_kind = quote!(const _: fn() = || {
            let _: <#owner as ::studio_db_api::PartitionedTable>::Kind = #kind;
            let _: Option<<#owner as ::studio_db_api::PartitionedTable>::PartitionKey> = None::<(#(#key_types,)*)>;
        };);
        partition_impl = quote!(#typed_kind impl ::studio_db_api::PartitionTable for #name {
            type Physical = #owner;
            const LIST: &'static str = ::studio_db_api::static_sql!(#select, " WHERE ", #partition_predicate);
        });
        ddl = quote!(<#owner as ::studio_db_api::Table>::DDL);
        columns = quote!(<#owner as ::studio_db_api::Table>::COLUMNS);
        get = quote!(::studio_db_api::static_sql!(#get, " AND ", #partition_predicate));
        delete = quote!(::studio_db_api::static_sql!(#delete, " AND ", #partition_predicate));
        insert = quote!(
            ::studio_db_api::static_sql!("INSERT INTO \"", <#owner as ::studio_db_api::Table>::NAME, "\"(\"", <#owner as ::studio_db_api::PartitionedTable>::KIND_COLUMN, "\",", #write_columns, ") VALUES(", #enum_owner::#code, ",", #placeholders, ")")
        );
        let conflict = quote!(
            ::studio_db_api::static_sql!(" ON CONFLICT(\"", <#owner as ::studio_db_api::PartitionedTable>::KIND_COLUMN, "\",", #key_columns, ")")
        );
        let action = if updates.is_empty() {
            " DO NOTHING".to_owned()
        } else {
            format!(" DO UPDATE SET {}", updates.join(","))
        };
        upsert = quote!(::studio_db_api::static_sql!(#insert, #conflict, #action));
        insert_new = quote!(::studio_db_api::static_sql!(#insert, #conflict, " DO NOTHING"));
        update_existing =
            quote!(::studio_db_api::static_sql!(#update_existing, " AND ", #partition_predicate));
        state_prefix = quote!(::studio_db_api::static_sql!(#partition_predicate, " AND "));
    }
    let generated_key_binds = generated_keys
        .iter()
        .map(|f| {
            let field = &f.name;
            quote!(&self.#field as &dyn ::studio_db_api::rusqlite::types::ToSql)
        })
        .collect::<Vec<_>>();
    let mutable = if receipt || writable.is_empty() || partitioned_by.is_some() {
        quote!()
    } else {
        quote!(impl ::studio_db_api::MutableTable for #name {
            const UPSERT: &'static str = #upsert;
            const INSERT_NEW: &'static str = #insert_new;
            const UPDATE_EXISTING: &'static str = #update_existing;
            fn bind_update<T>(&self, _connection: &::studio_db_api::rusqlite::Connection, f: impl FnOnce(&[&dyn ::studio_db_api::rusqlite::types::ToSql]) -> ::studio_db_api::rusqlite::Result<T>) -> ::studio_db_api::rusqlite::Result<T> {
                #(#encoded)* f(&[#(#binds,)* #(#generated_key_binds),*])
            }
        })
    };
    let revision = if let Some(field) = fields
        .iter()
        .find(|f| Some(&f.name) == revision_field.as_ref())
    {
        if receipt || field.generated || field.codec.is_some() || field.key {
            return Err(syn::Error::new_spanned(
                &field.name,
                "revision must be a writable non-key native column",
            ));
        }
        let ty = &field.ty;
        let predicate = format!(" AND {}=?", quoted(&field.column));
        Some(quote!(impl ::studio_db_api::RevisionTable for #name {
            type Revision = #ty;
            const UPDATE_AT_REVISION: &'static str = ::studio_db_api::static_sql!(#update_existing, #predicate);
        }))
    } else {
        None
    };
    let insertable = partitioned_by
        .is_none()
        .then(|| quote!(impl ::studio_db_api::InsertTable for #name {}));
    let sql_table = sql(&sql_table);
    Ok(quote! {
        const _: () = { #(#key_assertions)* };
        #partition_impl
        #insertable
        #revision
        #(#markers)*
        impl #name { #(#constants)* }
        impl ::studio_db_api::Table for #name {
            type Database = #database;
            type Key = (#(#key_types,)*);
            const NAME: &'static str = #table_name_expr;
            const DDL: &'static str = #ddl;
            const INDEXES: &'static [::studio_db_api::IndexDescriptor] = &[#(#index_sql),*];
            const SELECT: &'static str = #table_select;
            const PARTITION: &'static str = #partition_predicate;
            const STATE_CHECK: &'static str = ::studio_db_api::static_sql!("SELECT count(*) FROM ", #sql_table, " WHERE ", #state_prefix, "NOT (", #constraints, ")");
            const VALID_ROWS: &'static str = ::studio_db_api::static_sql!(#select, " WHERE ", #state_prefix, "(", #constraints, ") IS NOT FALSE");
            const GET: &'static str = #get;
            const INSERT: &'static str = #insert;
            const DELETE: &'static str = #delete;
            const COLUMNS: &'static [::studio_db_api::ColumnDescriptor] = #columns;
            fn read(_connection: &::studio_db_api::rusqlite::Connection, row: &::studio_db_api::rusqlite::Row<'_>) -> ::studio_db_api::rusqlite::Result<Self> { let value = Self { #(#reads),* }; ::studio_db_api::Table::validate(&value)?; Ok(value) }
            #validate
            #state_check
            fn bind<T>(&self, _connection: &::studio_db_api::rusqlite::Connection, f: impl FnOnce(&[&dyn ::studio_db_api::rusqlite::types::ToSql]) -> ::studio_db_api::rusqlite::Result<T>) -> ::studio_db_api::rusqlite::Result<T> { #(#encoded)* f(&[#(#binds),*]) }
            fn bind_key<T>(key: &Self::Key, f: impl FnOnce(&[&dyn ::studio_db_api::rusqlite::types::ToSql]) -> ::studio_db_api::rusqlite::Result<T>) -> ::studio_db_api::rusqlite::Result<T> { f(&[#(#key_binds),*]) }
        }
        #mutable
        #update
    })
}
fn predicate_sql(expr: &Expr, fields: &[Field]) -> syn::Result<Tokens> {
    if let Expr::Lit(lit) = expr
        && let syn::Lit::Str(sql) = &lit.lit
    {
        return Ok(quote!(#sql));
    }
    if let Expr::MethodCall(call) = expr {
        let Expr::Path(receiver) = call.receiver.as_ref() else {
            return Err(syn::Error::new_spanned(
                expr,
                "predicate receiver must name a field",
            ));
        };
        let field = fields
            .iter()
            .find(|f| receiver.path.is_ident(&f.name))
            .ok_or_else(|| syn::Error::new_spanned(receiver, "unknown predicate field"))?;
        let column = &field.column;
        let op = match call.method.to_string().as_str() {
            "eq" => " = ",
            "ne" => " != ",
            "lt" => " < ",
            "le" => " <= ",
            "gt" => " > ",
            "ge" => " >= ",
            "is_null" if call.args.is_empty() => return Ok(quote!(concat!(#column," IS NULL"))),
            "is_not_null" if call.args.is_empty() => {
                return Ok(quote!(concat!(#column," IS NOT NULL")));
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    &call.method,
                    "unsupported static predicate",
                ));
            }
        };
        if call.args.len() != 1 {
            return Err(syn::Error::new_spanned(
                call,
                "comparison requires one value",
            ));
        }
        let argument = &call.args[0];
        let ty = &field.ty;
        let value = argument;
        let value = if let Expr::Path(path) = value {
            let mut owner = path.path.clone();
            let variant = owner
                .segments
                .pop()
                .expect("path segment")
                .into_value()
                .ident;
            owner.segments.pop_punct();
            let code = format_ident!("DB_CODE_{}", variant.to_string().to_uppercase());
            quote!(#owner::#code)
        } else {
            let code = integer(value)?.to_string();
            quote!(#code)
        };
        return Ok(
            quote!({ const _: fn() = || { let _: #ty = #argument; }; ::studio_db_api::static_sql!(#column,#op,#value) }),
        );
    }
    Err(syn::Error::new_spanned(
        expr,
        "expected a SQL string or field comparison",
    ))
}

/// Execute a single-table query with compile-time SQL and column-typed bindings.
#[proc_macro]
pub fn db_query(input: TokenStream) -> TokenStream {
    expand_statement(parse_macro_input!(input as QueryInput), Statement::Select)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro]
pub fn db_update(input: TokenStream) -> TokenStream {
    expand_statement(parse_macro_input!(input as QueryInput), Statement::Update)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro]
pub fn db_delete(input: TokenStream) -> TokenStream {
    expand_statement(parse_macro_input!(input as QueryInput), Statement::Delete)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Statement {
    Select,
    Update,
    Delete,
}
enum Selection {
    Rows,
    Project(Path, bool),
    Scalar(Path, bool),
    Scalars(Path),
    Exists,
}
struct QueryInput {
    tx: Expr,
    row: Path,
    filters: Vec<Expr>,
    order: Vec<Expr>,
    limit: Option<Expr>,
    assignments: Vec<Expr>,
    selection: Selection,
}
impl syn::parse::Parse for QueryInput {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let tx = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let row = input.parse()?;
        let mut query = Self {
            tx,
            row,
            filters: vec![],
            order: vec![],
            limit: None,
            assignments: vec![],
            selection: Selection::Rows,
        };
        let mut seen = Vec::new();
        while !input.is_empty() {
            input.parse::<syn::Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let clause: Ident = input.parse()?;
            if seen.contains(&clause.to_string()) {
                return Err(syn::Error::new_spanned(clause, "duplicate query clause"));
            }
            seen.push(clause.to_string());
            let content;
            syn::parenthesized!(content in input);
            let selection = match clause.to_string().as_str() {
                "project" | "project_one" => Some(Selection::Project(
                    content.parse()?,
                    clause == "project_one",
                )),
                "scalar" | "scalar_raw" => {
                    Some(Selection::Scalar(content.parse()?, clause == "scalar_raw"))
                }
                "scalars" => Some(Selection::Scalars(content.parse()?)),
                "exists" if content.is_empty() => Some(Selection::Exists),
                _ => None,
            };
            if let Some(selection) = selection {
                if !matches!(query.selection, Selection::Rows) {
                    return Err(syn::Error::new_spanned(
                        clause,
                        "only one selection clause is allowed",
                    ));
                }
                query.selection = selection;
                continue;
            }
            let args = content
                .parse_terminated(Expr::parse, syn::Token![,])?
                .into_iter()
                .collect::<Vec<_>>();
            match clause.to_string().as_str() {
                "filter" => query.filters = args,
                "set" => query.assignments = args,
                "order_by" => query.order = args,
                "limit" if args.len() == 1 => query.limit = args.into_iter().next(),
                _ => {
                    return Err(syn::Error::new_spanned(
                        clause,
                        "expected filter(...), order_by(...), or limit(u32)",
                    ));
                }
            }
        }
        Ok(query)
    }
}
fn query_column(expr: &Expr) -> syn::Result<&Path> {
    if let Expr::Path(path) = expr {
        Ok(&path.path)
    } else {
        Err(syn::Error::new_spanned(expr, "expected Row::COLUMN"))
    }
}
fn expand_statement(query: QueryInput, kind: Statement) -> syn::Result<Tokens> {
    let QueryInput {
        tx,
        row,
        filters,
        order,
        limit,
        assignments,
        selection,
    } = query;
    if kind != Statement::Select && (!order.is_empty() || limit.is_some()) {
        return Err(syn::Error::new_spanned(
            &row,
            "mutations accept only set and filter clauses",
        ));
    }
    if (kind == Statement::Update) == assignments.is_empty() {
        return Err(syn::Error::new_spanned(
            &row,
            "set(...) is required only for updates",
        ));
    }
    if !matches!(selection, Selection::Rows) && kind != Statement::Select {
        return Err(syn::Error::new_spanned(&row, "selection is read-only"));
    }
    if matches!(
        selection,
        Selection::Project(_, true) | Selection::Scalar(..) | Selection::Exists
    ) && limit.is_some()
    {
        return Err(syn::Error::new_spanned(
            &row,
            "single-row selection does not take a limit",
        ));
    }
    let select = match &selection {
        Selection::Rows => quote!(<#row as ::studio_db_api::Table>::SELECT),
        Selection::Project(p, _) => quote!(<#p as ::studio_db_api::Projection>::SELECT),
        Selection::Scalar(col, _) | Selection::Scalars(col) => quote!(
            ::studio_db_api::static_sql!("SELECT ", ::studio_db_api::column_name::<#row,_>(#col), " FROM \"", <#row as ::studio_db_api::Table>::NAME, "\"", if <#row as ::studio_db_api::Table>::PARTITION.is_empty() { "" } else { " WHERE " }, <#row as ::studio_db_api::Table>::PARTITION)
        ),
        Selection::Exists => quote!(
            ::studio_db_api::static_sql!("SELECT 1 FROM \"", <#row as ::studio_db_api::Table>::NAME, "\"", if <#row as ::studio_db_api::Table>::PARTITION.is_empty() { "" } else { " WHERE " }, <#row as ::studio_db_api::Table>::PARTITION)
        ),
    };
    let mut sql = match kind {
        Statement::Select => vec![select],
        Statement::Update => vec![
            quote!("UPDATE \""),
            quote!(<#row as ::studio_db_api::Table>::NAME),
            quote!("\" SET "),
        ],
        Statement::Delete => vec![
            quote!("DELETE FROM \""),
            quote!(<#row as ::studio_db_api::Table>::NAME),
            quote!("\""),
        ],
    };
    let mut values = Vec::new();
    let mut bindings = Vec::new();
    for (i, expr) in assignments.iter().enumerate() {
        let Expr::Assign(assignment) = expr else {
            return Err(syn::Error::new_spanned(
                expr,
                "expected Row::COLUMN = &value",
            ));
        };
        let col = query_column(&assignment.left)?;
        if i > 0 {
            sql.push(quote!(", "));
        }
        sql.push(quote!(::studio_db_api::writable_column_name::<#row, _>(#col)));
        sql.push(quote!(" = ?"));
        let argument = &assignment.right;
        let input = format_ident!("__db_set_input_{i}");
        let value = format_ident!("__db_set_value_{i}");
        values.push(quote!(let #input = #argument; let #value = __db_tx.read().parameter::<#row, _>(#col, #input)?;));
        bindings.push(quote!(&#value as &dyn ::studio_db_api::rusqlite::types::ToSql));
    }
    if kind != Statement::Select {
        sql.push(
        quote!(if <#row as ::studio_db_api::Table>::PARTITION.is_empty() { "" } else { " WHERE " }),
    );
        sql.push(quote!(<#row as ::studio_db_api::Table>::PARTITION));
    }
    for (i, expr) in filters.iter().enumerate() {
        let Expr::MethodCall(call) = expr else {
            return Err(syn::Error::new_spanned(
                expr,
                "expected Column.eq(&value), in_(&slice), or a null predicate",
            ));
        };
        let col = query_column(&call.receiver)?;
        sql.push(if i == 0 {
            quote!(if <#row as ::studio_db_api::Table>::PARTITION.is_empty() { " WHERE " } else { " AND " })
        } else {
            quote!(" AND ")
        });
        sql.push(quote!(::studio_db_api::column_name::<#row, _>(#col)));
        let method = call.method.to_string();
        if method == "is_null" || method == "is_not_null" {
            if !call.args.is_empty() {
                return Err(syn::Error::new_spanned(
                    call,
                    "null predicates take no arguments",
                ));
            }
            let predicate = if method == "is_null" {
                " IS NULL"
            } else {
                " IS NOT NULL"
            };
            sql.push(quote!(#predicate));
            continue;
        }
        if call.args.len() != 1 {
            return Err(syn::Error::new_spanned(
                call,
                "comparison takes exactly one typed reference",
            ));
        }
        let argument = &call.args[0];
        if method == "in_" {
            sql.push(quote!(" IN rarray(?)"));
            let value = format_ident!("__db_array_{i}");
            values.push(
                quote!(let #value = __db_tx.read().array_parameter::<#row, _>(#col, #argument)?;),
            );
            bindings.push(quote!(&#value as &dyn ::studio_db_api::rusqlite::types::ToSql));
        } else {
            let borrowed = method.ends_with("_ref");
            let operation = method.strip_suffix("_ref").unwrap_or(&method);
            let parameter = if borrowed {
                format_ident!("borrowed_parameter")
            } else {
                format_ident!("parameter")
            };
            let operator = match operation {
                "eq" => " = ?",
                "ne" => " != ?",
                "lt" => " < ?",
                "le" => " <= ?",
                "gt" => " > ?",
                "ge" => " >= ?",
                _ => {
                    return Err(syn::Error::new_spanned(
                        &call.method,
                        "unsupported comparison",
                    ));
                }
            };
            sql.push(quote!(#operator));
            let value = format_ident!("__db_parameter_{i}");
            let input = format_ident!("__db_input_{i}");
            values.push(quote!(let #input = #argument; let #value = __db_tx.read().#parameter::<#row, _>(#col, #input)?;));
            bindings.push(quote!(&#value as &dyn ::studio_db_api::rusqlite::types::ToSql));
        }
    }
    for (i, expr) in order.iter().enumerate() {
        let Expr::Field(field) = expr else {
            return Err(syn::Error::new_spanned(
                expr,
                "expected Row::COLUMN.asc or .desc",
            ));
        };
        let col = query_column(&field.base)?;
        let direction = match &field.member {
            syn::Member::Named(name) if name == "asc" => " ASC",
            syn::Member::Named(name) if name == "desc" => " DESC",
            syn::Member::Named(_) | syn::Member::Unnamed(_) => {
                return Err(syn::Error::new_spanned(expr, "expected asc or desc"));
            }
        };
        sql.push(if i == 0 {
            quote!(" ORDER BY ")
        } else {
            quote!(", ")
        });
        sql.push(quote!(::studio_db_api::column_name::<#row, _>(#col)));
        sql.push(quote!(#direction));
    }
    if let Some(limit) = limit {
        // A direct LIMIT variable makes SQLite expire the VM on every rebind.
        sql.push(quote!(" LIMIT CAST(? AS INTEGER)"));
        values.push(quote!(let __db_limit: i64 = ::std::convert::Into::into(#limit);));
        bindings.push(quote!(
            &__db_limit as &dyn ::studio_db_api::rusqlite::types::ToSql
        ));
    }
    let borrow = if kind == Statement::Select {
        quote!(&(#tx))
    } else {
        quote!(&mut (#tx))
    };
    let execute = match selection {
        Selection::Project(p, one) => {
            let method = if one {
                format_ident!("project_one")
            } else {
                format_ident!("project")
            };
            quote!({
                fn assert_owner<P: ::studio_db_api::Projection<Table = #row>>() {} assert_owner::<#p>();
                __db_tx.read().#method::<#p>(SQL, &[#(#bindings),*])
            })
        }
        Selection::Scalar(col, raw) => {
            let method = if raw {
                format_ident!("scalar_raw")
            } else {
                format_ident!("scalar")
            };
            quote!(__db_tx.read().#method::<#row,_>(#col, SQL, &[#(#bindings),*]))
        }
        Selection::Scalars(col) => {
            quote!(__db_tx.read().scalars::<#row,_>(#col, SQL, &[#(#bindings),*]))
        }
        Selection::Exists => quote!(__db_tx.read().exists::<#row>(SQL, &[#(#bindings),*])),
        Selection::Rows => match kind {
            Statement::Select => quote!(__db_tx.query::<#row>(SQL, &[#(#bindings),*])),
            Statement::Update => quote!(__db_tx.update::<#row>(SQL, &[#(#bindings),*])),
            Statement::Delete => quote!(__db_tx.execute::<#row>(SQL, &[#(#bindings),*])),
        },
    };
    // Input clause order never affects legal SQL clause order.
    Ok(quote!((|| -> ::studio_db_api::rusqlite::Result<_> {
        const SQL: &str = ::studio_db_api::static_sql!(#(#sql),*);
        let __db_tx = #borrow;
        #(#values)*
        #execute
    })()))
}

#[proc_macro_derive(DbProjection, attributes(db))]
pub fn projection(input: TokenStream) -> TokenStream {
    expand_projection(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
fn expand_projection(input: &DeriveInput) -> syn::Result<Tokens> {
    let name = &input.ident;
    let mut owner = None::<Path>;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("db")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("of") {
                owner = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("expected of = Row"))
            }
        })?;
    }
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "projection requires a concrete type",
        ));
    }
    let owner = owner.ok_or_else(|| syn::Error::new_spanned(name, "missing of = Row"))?;
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "projection requires a struct",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "projection requires named fields",
        ));
    };
    if fields.named.is_empty() {
        return Err(syn::Error::new_spanned(name, "projection requires a field"));
    }
    let mut sql = vec![quote!("SELECT ")];
    let mut reads = Vec::new();
    for (i, field) in fields.named.iter().enumerate() {
        let field_name = field.ident.as_ref().expect("named field");
        let mut column = format_ident!("{}", field_name.to_string().to_uppercase());
        let mut raw = false;
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("db")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("field") {
                    let name: Ident = meta.value()?.parse()?;
                    column = format_ident!("{}", name.to_string().to_uppercase());
                    Ok(())
                } else if meta.path.is_ident("raw") {
                    raw = true;
                    Ok(())
                } else {
                    Err(meta.error("expected raw"))
                }
            })?;
        }
        if i > 0 {
            sql.push(quote!(","));
        }
        sql.push(quote!(::studio_db_api::column_name::<#owner, _>(#owner::#column)));
        let read = if raw {
            quote!(::studio_db_api::projection_raw(row, #i, #owner::#column)?)
        } else {
            quote!(::studio_db_api::projection_value(connection, row, #i, #owner::#column)?)
        };
        reads.push(quote!(#field_name: #read));
    }
    sql.extend([
        quote!(" FROM \""),
        quote!(<#owner as ::studio_db_api::Table>::NAME),
        quote!("\""),
    ]);
    sql.push(quote!(if <#owner as ::studio_db_api::Table>::PARTITION.is_empty() { "" } else { " WHERE " }));
    sql.push(quote!(<#owner as ::studio_db_api::Table>::PARTITION));
    Ok(quote!(impl ::studio_db_api::Projection for #name {
        type Table = #owner;
        const SELECT: &'static str = ::studio_db_api::static_sql!(#(#sql),*);
        fn read(connection: &::studio_db_api::rusqlite::Connection, row: &::studio_db_api::rusqlite::Row<'_>) -> ::studio_db_api::rusqlite::Result<Self> { let _ = connection; Ok(Self { #(#reads),* }) }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiers_reject_all_sqlite_keywords_and_non_lowercase_names() {
        let span: LitStr = syn::parse_quote!("identifier");
        for keyword in SQL_KEYWORDS.split_ascii_whitespace() {
            assert!(identifier(keyword, &span).is_err());
        }
        for invalid in ["", "CamelCase", "a-b", "0id", "é"] {
            assert!(identifier(invalid, &span).is_err());
        }
        for valid in ["id", "_id", "column_2"] {
            assert!(identifier(valid, &span).is_ok());
        }
    }
    #[test]
    fn rejects_unstable_enum_codes_and_invalid_table_descriptors() {
        for input in [
            syn::parse_quote!(
                enum Missing {
                    A,
                }
            ),
            syn::parse_quote!(
                enum Open {
                    A(String) = 1,
                }
            ),
            syn::parse_quote!(
                enum Expression {
                    A = 1 + 2,
                }
            ),
        ] {
            assert!(expand_enum(&input).is_err());
        }
        for input in [
            syn::parse_quote!(
                #[db(table="bad-name",database=Canvas)]
                struct Bad {
                    #[db(key)]
                    id: i64,
                }
            ),
            syn::parse_quote!(
                #[db(table="bad",database=Canvas)]
                struct Bad {
                    id: i64,
                }
            ),
            syn::parse_quote!(
                #[db(table="bad",database=Canvas,index(name="x",columns(unknown)))]
                struct Bad {
                    #[db(key)]
                    id: i64,
                }
            ),
            syn::parse_quote!(
                #[db(table="bad",database=Canvas)]
                struct Bad {
                    #[db(key)]
                    id: i64,
                    #[db(json)]
                    body: String,
                }
            ),
        ] {
            assert!(expand_table(&input).is_err());
        }
    }
}

use proc_macro_error::{abort, emit_warning, proc_macro_error};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Data, DeriveInput, Ident, Meta, parse_macro_input};

fn snake_case(s: &str) -> String {
    s.chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if c.is_uppercase() {
                if i > 0 {
                    vec!['_', c.to_ascii_lowercase()]
                } else {
                    vec![c.to_ascii_lowercase()]
                }
            } else {
                vec![c]
            }
        })
        .collect()
}

#[derive(Debug)]
struct Index {
    name: Ident,
    path: TokenStream,
}
struct StructArgs {
    get_store: TokenStream,
    key: TokenStream,
    key_path: TokenStream,
    indices: Vec<Index>,
}
impl StructArgs {
    fn from_attrs(input: &DeriveInput) -> Self {
        let mut store = TokenStream::new();
        let mut key = TokenStream::new();
        let mut key_path = TokenStream::new();
        let mut indices = Vec::new();
        // parse attributes on the struct
        for attr in &input.attrs {
            let Meta::List(ref meta_list) = attr.meta else {
                continue;
            };
            if !meta_list.path.is_ident("unistore") {
                continue;
            }
            let inner = meta_list
                .parse_args::<Meta>()
                .expect("Failed to parse unistore attribute");
            match inner {
                // Check for `#[unistore(store = ...)]` attribute
                Meta::NameValue(nv) if nv.path.is_ident("store") => {
                    store = nv.value.to_token_stream();
                }
                _ => emit_warning!(attr, "Unsupported unistore attribute"),
            }
        }
        let Data::Struct(struc) = &input.data else {
            abort!(input.ident, "UniStoreItem can only be derived for structs");
        };
        // parse attributes on the fields
        for field in &struc.fields {
            for attr in &field.attrs {
                let Meta::List(ref meta_list) = attr.meta else {
                    continue;
                };
                if !meta_list.path.is_ident("unistore") {
                    continue;
                }
                let inner = meta_list
                    .parse_args::<Meta>()
                    .expect("Failed to parse unistore attribute");
                match inner {
                    // Check for `#[unistore(key)]` attribute
                    Meta::Path(p) if p.is_ident("key") => {
                        if key.is_empty() {
                            key = field.ty.to_token_stream();
                            field.ident.as_ref().unwrap_or_else(|| {
                                abort!(field, "Field must have an identifier to be used as an key")
                            });
                            key_path = get_field_path(field);
                        } else {
                            abort!(
                                field.ident,
                                "Only one field can be marked with #[unistore(key)]"
                            );
                        }
                    }
                    // Check for `#[unistore(index)]` attribute
                    Meta::Path(p) if p.is_ident("index") => {
                        let name = field.ident.clone().unwrap_or_else(|| {
                            abort!(
                                field,
                                "Field must have an identifier to be used as an index"
                            )
                        });
                        let path = get_field_path(field);
                        indices.push(Index { name, path });
                    }
                    _ => emit_warning!(attr, "Unsupported unistore attribute"),
                }
            }
        }
        if store.is_empty() {
            abort!(
                input.ident,
                "Expected #[unistore(store = ...)] attribute on the struct"
            )
        }
        if key.is_empty() {
            abort!(
                input.ident,
                "Expected #[unistore(key)] attribute on a field"
            )
        }
        StructArgs {
            get_store: store,
            key,
            key_path,
            indices,
        }
    }
}

fn get_field_path(field: &syn::Field) -> TokenStream {
    let field_ident = field.ident.as_ref().unwrap_or_else(|| {
        abort!(
            field,
            "Field must have an identifier to be used as index or key"
        )
    });
    if is_copy(&field.ty) {
        quote! { self.#field_ident }
    } else if let syn::Type::Path(tp) = &field.ty {
        if tp.path.segments.last().is_some_and(|s| s.ident == "String") {
            quote! { self.#field_ident.as_str() }
        } else {
            quote! { self.#field_ident.clone() }
        }
    } else {
        quote! { self.#field_ident.clone() }
    }
}

fn is_copy(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                segment.ident == "u8"
                    || segment.ident == "i8"
                    || segment.ident == "u16"
                    || segment.ident == "i16"
                    || segment.ident == "u32"
                    || segment.ident == "i32"
                    || segment.ident == "f32"
                    || segment.ident == "u64"
                    || segment.ident == "i64"
                    || segment.ident == "f64"
                    || segment.ident == "bool"
            } else {
                false
            }
        }
        _ => false,
    }
}

#[proc_macro_derive(UniStoreItem, attributes(unistore))]
#[proc_macro_error]
pub fn derive_unistore_item(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struc = &input.ident;
    let name = snake_case(&struc.to_string());
    let StructArgs {
        get_store,
        key,
        key_path,
        indices,
    } = StructArgs::from_attrs(&input);

    let key_table = impl_table(
        &key,
        &struc.to_token_stream(),
        &name.to_token_stream(),
        &get_store,
    );

    let index_tables = indices.iter().map(|index| {
        let name = snake_case(&index.name.to_string()).to_token_stream();
        let table = impl_index(&name, &key, &struc.to_token_stream());
        quote! {
            #name => {
                #table
            }
        }
    });

    let get_index = if indices.is_empty() {
        TokenStream::new()
    } else {
        quote! {
            async fn index_table(
                index: &'static str,
            ) -> Result<&'static unistore::UniIndex<'static, String, Self::Key, Self>, unistore::Error>
            {
                match index {
                    #(#index_tables)*
                    _ => Err(unistore::Error::MissingIndex(index)),
                }
            }
        }
    };

    let insert_indices = if indices.is_empty() {
        quote! {}
    } else {
        let insertions = indices.iter().map(|index| {
            let name = snake_case(&index.name.to_string()).to_token_stream();
            let path = &index.path;
            quote! {
                let index_table = Self::index_table(#name).await?;
                index_table.insert(#path, self.unistore_key()).await?;
            }
        });
        quote! {
            async fn insert_indices(&self) -> Result<(), unistore::Error> {
                #(#insertions)*
                Ok(())
            }
        }
    };

    let index_getters = indices.iter().map(|index| {
        let name = snake_case(&index.name.to_string()).to_token_stream();
        let fn_name = format_ident!("get_by_{}", index.name);
        let fn_name_first = format_ident!("get_first_by_{}", index.name);
        quote! {
            pub async fn #fn_name(value: &str) -> Result<Vec<(#key, Self)>, unistore::Error> {
                let index_table = Self::index_table(#name).await?;
                index_table.get(value).await
            }
            pub async fn #fn_name_first(value: &str) -> Result<Option<(#key, Self)>, unistore::Error> {
                let index_table = Self::index_table(#name).await?;
                index_table.get_first(value).await
            }
        }
    });

    let expanded = quote! {
        impl unistore::UniStoreItem for #struc {
            type Key = #key;

            async fn table() -> &'static unistore::UniTable<'static, #key, #struc> {
                #key_table
            }

            #get_index

            #insert_indices

            fn unistore_key(&self) -> Self::Key {
                #key_path
            }
        }

        impl #struc{
            #(#index_getters)*
        }
    };
    proc_macro::TokenStream::from(expanded)
}

fn impl_table(
    key: &TokenStream,
    val: &TokenStream,
    name: &TokenStream,
    get_store: &TokenStream,
) -> TokenStream {
    quote! {
        static TABLE: std::sync::OnceLock<unistore::UniTable<'static, #key, #val>> =
            std::sync::OnceLock::new();
        static INITIALIZING: unistore::Mutex<()> = unistore::Mutex::new(());

        if let Some(table) = TABLE.get() {
            return table;
        }
        let _lock = INITIALIZING.lock().await;
        if let Some(table) = TABLE.get() {
            return table;
        }
        let store = #get_store().await;
        let table = store.create_table(#name, true).await.expect("Failed to create table");
        TABLE.set(table).expect("Failed to set table");
        TABLE.get().unwrap()
    }
}

fn impl_index(name: &TokenStream, key: &TokenStream, val: &TokenStream) -> TokenStream {
    quote! {
        static INDEX: std::sync::OnceLock<unistore::UniIndex<'static, String, #key, #val>> =
            std::sync::OnceLock::new();
        static INITIALIZING: unistore::Mutex<()> = unistore::Mutex::new(());

        if let Some(index) = INDEX.get() {
            return Ok(index);
        }
        let _lock = INITIALIZING.lock().await;
        if let Some(index) = INDEX.get() {
            return Ok(index);
        }
        let table = Self::table().await;
        let index = table.create_index(#name).await.expect("Failed to create index");
        INDEX.set(index).expect("Failed to set table");
        Ok(INDEX.get().unwrap())
    }
}

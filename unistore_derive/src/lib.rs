use proc_macro_error::{abort, emit_warning, proc_macro_error};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Ident, Meta, parse_macro_input};

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

struct StructArgs {
    get_store: TokenStream,
    key: TokenStream,
    key_path: TokenStream,
}
impl StructArgs {
    fn from_attrs(input: &DeriveInput) -> Self {
        let mut store = TokenStream::new();
        let mut key = TokenStream::new();
        let mut key_path = TokenStream::new();
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
                            let field_ident = field.ident.as_ref().unwrap_or_else(|| {
                                abort!(field, "Field must have an identifier to be used as a key")
                            });
                            if is_copy(&field.ty) {
                                key_path = quote!(self.#field_ident);
                            } else {
                                key_path = quote!(self.#field_ident.clone());
                            }
                        } else {
                            abort!(
                                field.ident,
                                "Only one field can be marked with #[unistore(key)]"
                            );
                        }
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
        }
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
    } = StructArgs::from_attrs(&input);

    let expanded = quote! {
        impl unistore::UniStoreItem for #struc {
            type Key = #key;

            async fn table() -> &'static unistore::UniTable<'static, #key, #struc> {
                static TABLE: std::sync::OnceLock<unistore::UniTable<'static, #key, #struc>> =
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

            fn unistore_key(&self) -> Self::Key {
                #key_path
            }
        };
    };
    proc_macro::TokenStream::from(expanded)
}

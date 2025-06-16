#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

mod derive_tests {
    extern crate self as unistore;
    use crate::{UniStoreItem, static_store};
    use serde::{Deserialize, Serialize};

    static_store!(get_test_store, "com", "example", "unistore");

    #[derive(UniStoreItem, Serialize, Deserialize, PartialEq, Debug)]
    #[unistore(store = get_test_store)]
    struct Entry {
        #[unistore(key)]
        key: u32,
        value: String,
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    async fn test_derive() {
        let value = Entry {
            key: 1,
            value: "Hello, UniStore!".to_string(),
        };
        value.insert().await.expect("Failed to insert value");
        let retrieved = Entry::get(1).await.expect("Failed to get value");
        assert_eq!(retrieved, Some(value));
    }
}

mod index_tests {
    extern crate self as unistore;
    use crate::{UniStoreItem, index::UniIndex, static_store};
    use serde::{Deserialize, Serialize};

    static_store!(get_test_store, "com", "example", "unistore");

    #[derive(UniStoreItem, Serialize, Deserialize, PartialEq, Debug)]
    #[unistore(store = get_test_store)]
    struct IndexEntry {
        #[unistore(key)]
        key: u32,
        #[unistore(index)]
        name: String,
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    // #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    async fn test_derived_index() {
        let value = IndexEntry {
            key: 1,
            name: "One".to_string(),
        };
        value.insert().await.expect("Failed to insert value");
        let retrieved = IndexEntry::get(1).await.expect("Failed to get value");
        assert_eq!(retrieved.as_ref(), Some(&value));
        let index_retrieved = IndexEntry::get_by_index("name", "One")
            .await
            .expect("Failed to get value by index");
        assert_eq!(index_retrieved, vec![(1, value)]);
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    async fn test_create_index() {
        let value = IndexEntry {
            key: 1,
            name: "One".to_string(),
        };
        let table = get_test_store()
            .await
            .create_table::<u32, IndexEntry>("index_test", false)
            .await
            .expect("Failed to create table");

        println!("Creating index");
        let index: UniIndex<String, _, _> = table
            .create_index("name")
            .await
            .expect("Failed to create index");
        println!("Inserting value into table");
        table
            .insert(value.unistore_key(), &value)
            .await
            .expect("Failed to insert value");
        println!("Inserting value into index");
        index
            .insert(value.name.as_str(), value.unistore_key())
            .await
            .expect("Failed to insert index entry");
        println!("Retrieving value by index");
        let index_retrieved = index
            .get("One")
            .await
            .expect("Failed to get value by index");
        assert_eq!(index_retrieved, vec![(1, value)]);
    }
}

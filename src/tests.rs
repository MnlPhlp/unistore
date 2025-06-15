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

    #[tokio::test]
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

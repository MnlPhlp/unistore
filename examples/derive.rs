use serde::{Deserialize, Serialize};
use tracing::Level;
use unistore::{Error, UniStoreItem, static_store};

static_store!(get_store, "com", "example", "unistore");

#[derive(UniStoreItem, Serialize, Deserialize, PartialEq, Debug)]
#[unistore(store = get_store)]
struct Entry {
    #[unistore(key)]
    id: u32,
    #[unistore(index)]
    name: String,
}

#[cfg(target_arch = "wasm32")]
fn main() {
    panic!("This example is not supported on wasm32. Please run it in a native environment.");
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    if let Err(e) = inner().await {
        eprintln!("Error: {e}");
    }
}

async fn inner() -> Result<(), Error> {
    let value = Entry {
        id: 1,
        name: "One".to_string(),
    };
    value.save().await?;
    let retrieved = Entry::get(1).await?;
    assert_eq!(retrieved.as_ref(), Some(&value));

    // the index can be used by the static method `get_by_index` with the index name and value
    let index_retrieved = Entry::get_by_index("name", "One").await?;
    assert_eq!(index_retrieved[0].0, 1);
    assert_eq!(&index_retrieved[0].1, &value);

    // the macro also generates a get method for each index
    let index_retrieved = Entry::get_by_name("One").await?;
    assert_eq!(index_retrieved[0].0, 1);
    assert_eq!(&index_retrieved[0].1, &value);

    // multiple entries can have the same index value
    // this is why the functions return a vector of tuples
    let value2 = Entry {
        id: 2,
        name: "One".to_string(),
    };
    value2.save().await?;
    let index_retrieved = Entry::get_by_name("One").await?;
    assert_eq!(index_retrieved.len(), 2);
    assert_eq!(index_retrieved[0].0, 1);
    assert_eq!(&index_retrieved[0].1, &value);
    assert_eq!(index_retrieved[1].0, 2);
    assert_eq!(&index_retrieved[1].1, &value2);

    Ok(())
}

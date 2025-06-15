use serde::{Deserialize, Serialize};
use tracing::Level;
use unistore::{Error, UniStoreItem, static_store};

static_store!(get_store, "com", "example", "unistore");

#[derive(UniStoreItem, Serialize, Deserialize, PartialEq, Debug)]
#[unistore(store = get_store)]
struct Entry {
    #[unistore(key)]
    id: u32,
    value: String,
}

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
        value: "Hello, UniStore!".to_string(),
    };
    value.insert().await?;
    let retrieved = Entry::get(1).await?;
    assert_eq!(retrieved, Some(value));
    Ok(())
}

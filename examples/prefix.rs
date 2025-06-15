use tracing::Level;
use unistore::{Error, UniStore};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    if let Err(e) = inner().await {
        eprintln!("Error: {e}");
    }
}

async fn inner() -> Result<(), Error> {
    let store = UniStore::new("com", "example", "unistore").await?;
    let table = store.create_table("prefix", false).await?;
    table
        .insert("key1".to_string(), "value1".to_string())
        .await?;
    table
        .insert("key2".to_string(), "value2".to_string())
        .await?;
    let values = table.get_prefix("key").await?;
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].1, "value1");
    Ok(())
}

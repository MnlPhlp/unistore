use unistore::{Error, UniStore};

#[tokio::main]
async fn main() {
    if let Err(e) = inner().await {
        eprintln!("Error: {e}");
    }
}

async fn inner() -> Result<(), Error> {
    let store = UniStore::new("example_store").await?;

    // Create a table with string keys and values
    let table = store.create_table("table1", false).await?;
    table
        .insert("key1".to_string(), "value1".to_string())
        .await?;
    let value = table.get("key1").await?;
    assert_eq!(value, Some("value1".to_string()));

    // use can create another table with different key and value types for the same store
    let table = store.create_table("table2", false).await?;
    println!("Table 2 rows: {}", table.len().await?);
    // use can use `&K` or `K` as the key type
    table.insert(1, 10).await?;
    table.insert(&2, 20).await?;
    let value = table.get(&1).await?;
    assert_eq!(value, Some(10));
    let value = table.get(2).await?;
    assert_eq!(value, Some(20));
    Ok(())
}

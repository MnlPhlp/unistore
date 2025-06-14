use std::rc::Rc;

use idb::{DatabaseEvent, Factory, ObjectStoreParams};
use serde::{Serialize, de::DeserializeOwned};
use serde_wasm_bindgen::Serializer;
use std::sync::Mutex;

use crate::UniStore;

thread_local! {
    static DBS: Mutex<Vec<Rc<idb::Database>>> = Mutex::new(Vec::new());
}

pub struct Database(u8);
impl Database {
    pub fn get_db(&self) -> Rc<idb::Database> {
        DBS.with(|dbs| {
            let dbs = dbs.lock().unwrap();
            dbs[self.0 as usize].clone() // Access the database by its index
        })
    }

    fn new(db: idb::Database) -> Self {
        let id = DBS.with(|dbs| {
            let mut dbs = dbs.lock().unwrap();
            dbs.push(Rc::new(db));
            (dbs.len() - 1) as u8 // Return the index of the new database
        });
        Database(id) // Placeholder, replace with actual logic to return a unique identifier or handle
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    Idb(#[from] idb::Error),
    Serde(#[from] serde_wasm_bindgen::Error),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "wasm::Error: {self:?}")
    }
}

pub async fn create_database(name: &str) -> Result<Database, Error> {
    // Get a factory instance from global scope
    let factory = Factory::new()?;

    // Create an open request for the database
    let mut open_request = factory.open(name, None).unwrap();

    // Add an upgrade handler for database
    let name = name.to_string();
    open_request.on_upgrade_needed(move |event| {
        // Get database instance from event
        let database = event.database().unwrap();

        // Prepare object store params
        let store_params = ObjectStoreParams::new();

        // Create object store
        let _ = database.create_object_store(&name, store_params).unwrap();
    });

    // `await` open request
    let db = open_request.await?;
    Ok(Database::new(db))
}

pub async fn insert<K: Serialize, V: Serialize>(
    store: &UniStore<K, V>,
    key: K,
    value: V,
) -> Result<(), Error> {
    let tx = store
        .db
        .get_db()
        .transaction(&[store.name.as_str()], idb::TransactionMode::ReadWrite)?;
    let store = tx.object_store(&store.name)?;
    let value = &value.serialize(&Serializer::json_compatible()).unwrap();
    let key = &key.serialize(&Serializer::json_compatible()).unwrap();
    store.add(value, Some(key))?.await?;
    tx.commit()?.await?;
    Ok(())
}

pub async fn contains<K: Serialize, V>(store: &UniStore<K, V>, key: K) -> Result<bool, Error> {
    let tx = store
        .db
        .get_db()
        .transaction(&[store.name.as_str()], idb::TransactionMode::ReadOnly)?;
    let store = tx.object_store(&store.name)?;
    let key = key.serialize(&Serializer::json_compatible()).unwrap();
    let result = store.get(key)?.await?;
    Ok(result.is_some())
}

pub async fn get<K: Serialize, V: DeserializeOwned>(
    store: &UniStore<K, V>,
    key: K,
) -> Result<Option<V>, Error> {
    let tx = store
        .db
        .get_db()
        .transaction(&[store.name.as_str()], idb::TransactionMode::ReadOnly)?;
    let store = tx.object_store(&store.name)?;
    let key = key.serialize(&Serializer::json_compatible()).unwrap();
    let result = store.get(key)?.await?;
    if let Some(value) = result {
        let value: V = serde_wasm_bindgen::from_value(value)?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

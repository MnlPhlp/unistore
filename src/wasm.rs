use std::rc::Rc;

use idb::{DatabaseEvent, Factory, ObjectStoreParams};
use serde_wasm_bindgen::Serializer;
use std::sync::Mutex;

use crate::{AsKey, UniStore, UniTable, Value};

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

    fn update(&self, db: idb::Database) {
        DBS.with(|dbs| {
            let mut dbs = dbs.lock().unwrap();
            dbs[self.0 as usize] = Rc::new(db); // Update the database at the index
        });
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
    KeyTypeMismatch(serde_wasm_bindgen::Error),
    ValueTypeMismatch(serde_wasm_bindgen::Error),
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
    let open_request = factory.open(name, None).unwrap();

    // `await` open request
    let db = open_request.await?;
    Ok(Database::new(db))
}

pub async fn create_table<'a, K: Key, V: Value>(
    store: &'a UniStore,
    name: &str,
    replace_if_incomatible: bool,
) -> Result<UniTable<'a, K, V>, Error> {
    let db = store.db.get_db();
    let mut replace = false;

    'exists_check: {
        if db.store_names().iter().any(|s| s == name) {
            // If the store already exists, check if the types match
            let tx = db.transaction(&[name], idb::TransactionMode::ReadOnly)?;
            let obj_store = tx.object_store(name)?;
            if let Some(cursor) = obj_store.open_cursor(None, None)?.await? {
                let key = cursor.key()?;
                let value = cursor.value()?;
                if let Err(e) = serde_wasm_bindgen::from_value::<K>(key) {
                    if replace_if_incomatible {
                        replace = true;
                        break 'exists_check; // If we are replacing, break to create a new store
                    }
                    return Err(Error::KeyTypeMismatch(e));
                }
                if let Err(e) = serde_wasm_bindgen::from_value::<V>(value) {
                    if replace_if_incomatible {
                        replace = true;
                        break 'exists_check; // If we are replacing, break to create a new store
                    }
                    return Err(Error::ValueTypeMismatch(e));
                }
            }
            return Ok(UniTable {
                store,
                name: name.to_string(),
                phantom: std::marker::PhantomData,
            });
        }
    }

    // else create a new object store
    let store_params = ObjectStoreParams::new();
    let version = db.version().expect("Failed to get database version");
    db.close();

    let mut open_request = Factory::new()?
        .open(&store.name, Some(version + 1))
        .unwrap();
    let name_string = name.to_string();
    open_request.on_upgrade_needed(move |event| {
        let edb = event.database().unwrap();
        if replace {
            edb.delete_object_store(&name_string).unwrap();
        }
        let _ = edb.create_object_store(&name_string, store_params).unwrap();
    });
    let new_db = open_request.await?;
    store.db.update(new_db);

    Ok(UniTable {
        store,
        name: name.to_string(),
        phantom: std::marker::PhantomData,
    })
}

pub async fn insert<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
    value: V,
) -> Result<(), Error> {
    let tx = table
        .store
        .db
        .get_db()
        .transaction(&[table.name.as_str()], idb::TransactionMode::ReadWrite)?;
    let store = tx.object_store(&table.name)?;
    let value = &value.serialize(&Serializer::json_compatible()).unwrap();
    let key = &key
        .as_key()
        .serialize(&Serializer::json_compatible())
        .unwrap();
    store.add(value, Some(key))?.await?;
    tx.commit()?.await?;
    Ok(())
}

pub async fn contains<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<bool, Error> {
    let tx = table
        .store
        .db
        .get_db()
        .transaction(&[table.name.as_str()], idb::TransactionMode::ReadOnly)?;
    let store = tx.object_store(&table.name)?;
    let key = key
        .as_key()
        .serialize(&Serializer::json_compatible())
        .unwrap();
    let result = store.get(key)?.await?;
    Ok(result.is_some())
}

pub async fn get<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<Option<V>, Error> {
    let tx = table
        .store
        .db
        .get_db()
        .transaction(&[table.name.as_str()], idb::TransactionMode::ReadOnly)?;
    let store = tx.object_store(&table.name)?;
    let key = key
        .as_key()
        .serialize(&Serializer::json_compatible())
        .unwrap();
    let result = store.get(key)?.await?;
    if let Some(value) = result {
        let value: V = serde_wasm_bindgen::from_value(value)?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

pub async fn len<K: Key, V: Value>(table: &UniTable<'_, K, V>) -> Result<usize, Error> {
    let tx = table
        .store
        .db
        .get_db()
        .transaction(&[table.name.as_str()], idb::TransactionMode::ReadOnly)?;
    let store = tx.object_store(&table.name)?;
    let count = store.count(None)?.await?;
    Ok(count as usize)
}

pub async fn remove<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<(), Error> {
    let tx = table
        .store
        .db
        .get_db()
        .transaction(&[table.name.as_str()], idb::TransactionMode::ReadWrite)?;
    let store = tx.object_store(&table.name)?;
    let key = key
        .as_key()
        .serialize(&Serializer::json_compatible())
        .unwrap();
    store.delete(key)?.await?;
    tx.commit()?.await?;
    Ok(())
}

pub async fn is_empty<K: Key, V: Value>(table: &UniTable<'_, K, V>) -> Result<bool, Error> {
    let tx = table
        .store
        .db
        .get_db()
        .transaction(&[table.name.as_str()], idb::TransactionMode::ReadOnly)?;
    let store = tx.object_store(&table.name)?;
    let count = store.count(None)?.await?;
    Ok(count == 0)
}

pub async fn get_prefix<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    prefix: impl AsKey<K>,
) -> Result<Vec<(K, V)>, Error> {
    todo!();
}

pub trait Key: serde::Serialize + serde::de::DeserializeOwned {}
impl<T: serde::Serialize + serde::de::DeserializeOwned> Key for T {}

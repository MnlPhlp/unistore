use std::rc::Rc;

use idb::{DatabaseEvent, Factory, ObjectStoreParams};
use serde_wasm_bindgen::Serializer;
use std::sync::Mutex;
use wasm_bindgen::JsValue;

use crate::{AsKey, AsValue, Key, UniStore, UniTable, Value};

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
    CrateError(String),
    ValueTypeMismatch(serde_wasm_bindgen::Error),
    NoCursor,
}
impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        Self::CrateError(e.to_string())
    }
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
    let mut db = open_request.await?;
    db.on_version_change(|event| event.database().expect("database").close());
    Ok(Database::new(db))
}

async fn with_transaction<FUNC, FUT, R>(
    db: &Database,
    store_names: &[&str],
    mode: idb::TransactionMode,
    f: FUNC,
) -> Result<R, Error>
where
    FUNC: FnOnce(Rc<idb::Transaction>) -> FUT,
    FUT: Future<Output = Result<R, Error>>,
{
    let tx = Rc::new(db.get_db().transaction(store_names, mode)?);
    let result = f(tx.clone()).await;
    let tx = Rc::into_inner(tx).expect("Transaction can not be borrowed in 'with_transaction'");
    tx.commit().unwrap().await.unwrap();
    result
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
            let cursor = with_transaction(
                &store.db,
                &[name],
                idb::TransactionMode::ReadOnly,
                |tx| async move {
                    let obj_store = tx.object_store(name)?;
                    let cursor = obj_store.open_cursor(None, None)?.await?;
                    Ok(cursor)
                },
            )
            .await?;
            if let Some(cursor) = cursor {
                let key = cursor.key()?;
                let value = cursor.value()?;
                if let Err(e) =
                    K::from_key_string(&key.as_string().expect("Key has to be a string"))
                {
                    if replace_if_incomatible {
                        replace = true;
                        break 'exists_check; // If we are replacing, break to create a new store
                    }
                    return Err(e.into());
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
    let mut new_db = open_request.await?;
    new_db.on_version_change(|event| event.database().expect("database").close());
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
    value: impl AsValue<V>,
) -> Result<(), Error> {
    with_transaction(
        &table.store.db,
        &[&table.name],
        idb::TransactionMode::ReadWrite,
        |tx| async move {
            let store = tx.object_store(&table.name)?;
            let value = &value.serialize(&Serializer::json_compatible()).unwrap();
            let key = JsValue::from_str(&key.as_key().to_key_string());
            store.put(value, Some(&key))?.await?;
            Ok(())
        },
    )
    .await?;
    Ok(())
}

pub async fn contains<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<bool, Error> {
    let key = JsValue::from_str(&key.as_key().to_key_string());
    let result = with_transaction(
        &table.store.db,
        &[&table.name],
        idb::TransactionMode::ReadOnly,
        |tx| async move {
            let store = tx.object_store(&table.name)?;
            let val = store.get(key)?.await?;
            Ok(val)
        },
    )
    .await?;
    Ok(result.is_some())
}

pub async fn get<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<Option<V>, Error> {
    let key = JsValue::from_str(&key.as_key().to_key_string());
    let result = with_transaction(
        &table.store.db,
        &[&table.name],
        idb::TransactionMode::ReadOnly,
        |tx| async move {
            let store = tx.object_store(&table.name)?;
            let result = store.get(key)?.await?;
            Ok(result)
        },
    )
    .await?;
    if let Some(value) = result {
        let value: V = serde_wasm_bindgen::from_value(value)?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

pub async fn len<K: Key, V: Value>(table: &UniTable<'_, K, V>) -> Result<usize, Error> {
    let count = with_transaction(
        &table.store.db,
        &[&table.name],
        idb::TransactionMode::ReadOnly,
        |tx| async move {
            let store = tx.object_store(&table.name)?;
            let count = store.count(None)?.await?;
            Ok(count)
        },
    )
    .await?;
    Ok(count as usize)
}

pub async fn remove<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<(), Error> {
    with_transaction(
        &table.store.db,
        &[&table.name],
        idb::TransactionMode::ReadWrite,
        |tx| async move {
            let store = tx.object_store(&table.name)?;
            let key = JsValue::from_str(&key.as_key().to_key_string());
            store.delete(key)?.await?;
            Ok(())
        },
    )
    .await?;
    Ok(())
}

pub async fn is_empty<K: Key, V: Value>(table: &UniTable<'_, K, V>) -> Result<bool, Error> {
    len(table).await.map(|count| count == 0)
}

pub async fn get_prefix<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    prefix: impl AsKey<K>,
) -> Result<Vec<(K, V)>, Error> {
    let key_string = prefix.as_key().to_key_string();
    let key = JsValue::from_str(&key_string);
    let successor = JsValue::from_str(&get_successor(&key_string));
    tracing::info!("Key: {key:?}, Successor: {successor:?}");
    let result = with_transaction(
        &table.store.db,
        &[&table.name],
        idb::TransactionMode::ReadOnly,
        |tx| async move {
            let store = tx.object_store(&table.name)?;
            let mut values = Vec::new();
            let cursor = store
                .open_cursor(
                    Some(idb::Query::KeyRange(idb::KeyRange::bound(
                        &key,
                        &successor,
                        None,
                        Some(true),
                    )?)),
                    None,
                )?
                .await?;
            let mut cursor = match cursor {
                Some(cursor) => cursor.into_managed(),
                None => return Ok(Vec::new()),
            };
            loop {
                let Some(key) = cursor.key()? else {
                    break;
                };
                let Some(value) = cursor.value()? else {
                    break;
                };
                values.push((key, value));
                if cursor.next(None).await.is_err() {
                    break;
                }
            }
            Ok(values)
        },
    )
    .await?;
    result
        .into_iter()
        .map(|(key, value)| {
            let key_str = key.as_string().expect("Key should be a string");
            let key = K::from_key_string(&key_str).map_err(Error::from)?;
            let value: V = serde_wasm_bindgen::from_value(value).map_err(Error::from)?;
            Ok((key, value))
        })
        .collect()
}

fn get_successor(val: &str) -> String {
    let bytes = &val[..val.len() - 1];
    let c = val.chars().last().unwrap();
    let next = std::char::from_u32(c as u32 + 1).unwrap_or(c);
    format!("{bytes}{next}")
}

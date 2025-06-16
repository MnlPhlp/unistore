use fjall::{Keyspace, PartitionCreateOptions, PartitionHandle, Slice};
use futures::{
    SinkExt,
    channel::{mpsc, oneshot},
    executor::block_on_stream,
};
use tracing::info;

use crate::{AsKey, AsValue, Key, UniStore, UniTable, Value};

pub type Table = PartitionHandle;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Fjall error: {0}")]
    Fjall(#[from] fjall::Error),
    #[error("Store is not initialized")]
    StoreNotInitialized,
    #[error("Sending to mpsc channel failed: {0}")]
    Mpsc(#[from] mpsc::SendError),
    #[error("Receiving from oneshot channel failed: {0}")]
    OneShot(#[from] oneshot::Canceled),
    #[error("RMP encoding error: {0}")]
    RmpEncode(#[from] rmp_serde::encode::Error),
    #[error("RMP decoding error: {0}")]
    RmpDecode(#[from] rmp_serde::decode::Error),
    #[error("Data directory not found")]
    DataDirNotFound,
}

fn get_path(qualifier: &str, organization: &str, application: &str) -> Result<String, Error> {
    let base_dirs = robius_directories::ProjectDirs::from(qualifier, organization, application)
        .ok_or(Error::DataDirNotFound)?;
    let data_dir = base_dirs.data_dir();
    let path = data_dir
        .join("unistore.fjall")
        .to_string_lossy()
        .to_string();
    info!("Storage path: {path}");
    Ok(path)
}

pub struct Database(mpsc::Sender<Action>);
impl Database {
    pub async fn create_table(&self, name: &str) -> Result<(PartitionHandle, bool), Error> {
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::CreateTable {
            name: name.to_string(),
            resp_tx,
        })
        .await?;
        resp_rx.await?
    }

    async fn is_table_empty(&self, table: PartitionHandle) -> Result<bool, Error> {
        tracing::info!("Checking if table is empty: {}", table.name);
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::IsTableEmpty { table, resp_tx }).await?;
        resp_rx.await?
    }

    async fn first_key_value(
        &self,
        table: PartitionHandle,
    ) -> Result<Option<(Slice, Slice)>, Error> {
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::FirstKeyValue { table, resp_tx }).await?;
        resp_rx.await?
    }

    async fn delete_table(&self, table: PartitionHandle) -> Result<(), Error> {
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::DeleteTable { table, resp_tx }).await?;
        resp_rx.await?
    }

    async fn contains(&self, table: PartitionHandle, key: Slice) -> Result<bool, Error> {
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::Contains {
            table,
            key,
            resp_tx,
        })
        .await?;
        resp_rx.await?
    }

    async fn insert(&self, table: PartitionHandle, key: Slice, value: Slice) -> Result<(), Error> {
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::Insert {
            table,
            key,
            value,
            resp_tx,
        })
        .await?;
        resp_rx.await?
    }

    async fn get(&self, table: PartitionHandle, key: Slice) -> Result<Option<Slice>, Error> {
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::Get {
            table,
            key,
            resp_tx,
        })
        .await?;
        resp_rx.await?
    }

    async fn len(&self, table: PartitionHandle) -> Result<usize, Error> {
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::Len { table, resp_tx }).await?;
        resp_rx.await?
    }

    async fn remove(&self, table: PartitionHandle, key: Slice) -> Result<(), Error> {
        let mut tx = self.0.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Action::Remove {
            table,
            key,
            resp_tx,
        })
        .await?;
        resp_rx.await?
    }
}

enum Action {
    CreateDb {
        qualifier: String,
        organization: String,
        application: String,
        resp_tx: oneshot::Sender<Result<(), Error>>,
    },
    CreateTable {
        name: String,
        resp_tx: oneshot::Sender<Result<(PartitionHandle, bool), Error>>,
    },
    IsTableEmpty {
        table: PartitionHandle,
        resp_tx: oneshot::Sender<Result<bool, Error>>,
    },
    FirstKeyValue {
        table: PartitionHandle,
        resp_tx: oneshot::Sender<Result<Option<(Slice, Slice)>, Error>>,
    },
    DeleteTable {
        table: PartitionHandle,
        resp_tx: oneshot::Sender<Result<(), Error>>,
    },
    Insert {
        table: PartitionHandle,
        key: Slice,
        value: Slice,
        resp_tx: oneshot::Sender<Result<(), Error>>,
    },
    Get {
        table: PartitionHandle,
        key: Slice,
        resp_tx: oneshot::Sender<Result<Option<Slice>, Error>>,
    },
    Contains {
        table: PartitionHandle,
        key: Slice,
        resp_tx: oneshot::Sender<Result<bool, Error>>,
    },
    Len {
        table: PartitionHandle,
        resp_tx: oneshot::Sender<Result<usize, Error>>,
    },
    Remove {
        table: PartitionHandle,
        key: Slice,
        resp_tx: oneshot::Sender<Result<(), Error>>,
    },
}
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::CreateDb {
                qualifier,
                organization,
                application,
                ..
            } => write!(f, "CreateDb({qualifier}.{organization}.{application})"),
            Action::CreateTable { name, .. } => write!(f, "CreateTable({name})"),
            Action::IsTableEmpty { .. } => write!(f, "IsTableEmpty"),
            Action::FirstKeyValue { .. } => write!(f, "FirstKeyValue"),
            Action::DeleteTable { .. } => write!(f, "DeleteTable"),
            Action::Insert {
                table, key, value, ..
            } => {
                write!(
                    f,
                    "Insert(table: {}, key: {:?}, value: {:?})",
                    table.name, key, value
                )
            }
            Action::Get { table, key, .. } => {
                write!(f, "Get(table: {}, key: {:?})", table.name, key)
            }
            Action::Contains { table, key, .. } => {
                write!(f, "Contains(table: {}, key: {:?})", table.name, key)
            }
            Action::Len { table, .. } => write!(f, "Count(table: {})", table.name),
            Action::Remove { table, key, .. } => {
                write!(f, "Remove(table: {}, key: {:?})", table.name, key)
            }
        }
    }
}

fn start_worker() -> mpsc::Sender<Action> {
    let (tx, rx) = mpsc::channel(16);
    std::thread::spawn(move || {
        let mut keyspace = None;
        for action in block_on_stream(rx) {
            let err = match action {
                Action::CreateDb {
                    qualifier,
                    organization,
                    application,
                    resp_tx: resp,
                } => {
                    let ks = get_path(&qualifier, &organization, &application)
                        .and_then(|path| fjall::Config::new(path).open().map_err(Error::Fjall));
                    let result = match ks {
                        Err(e) => Err(e),
                        Ok(ks) => {
                            keyspace = Some(ks);
                            Ok(())
                        }
                    };
                    resp.send(result).is_err()
                }
                Action::CreateTable {
                    name,
                    resp_tx: resp,
                } => resp
                    .send(handle_create_table(keyspace.as_mut(), &name))
                    .is_err(),
                Action::IsTableEmpty { table, resp_tx } => {
                    let result = table.is_empty().map_err(Error::Fjall);
                    resp_tx.send(result).is_err()
                }
                Action::FirstKeyValue { table, resp_tx } => {
                    let result = table.first_key_value().map_err(Error::Fjall);
                    resp_tx.send(result).is_err()
                }
                Action::DeleteTable { table, resp_tx } => {
                    let result = handle_delete_table(keyspace.as_mut(), table);
                    resp_tx.send(result).is_err()
                }
                Action::Insert {
                    table,
                    key,
                    value,
                    resp_tx,
                } => {
                    let result = table.insert(key, value).map_err(Error::Fjall);
                    resp_tx.send(result).is_err()
                }
                Action::Get {
                    table,
                    key,
                    resp_tx,
                } => {
                    let result = table.get(key).map_err(Error::Fjall);
                    resp_tx.send(result).is_err()
                }
                Action::Contains {
                    table,
                    key,
                    resp_tx,
                } => {
                    let result = table.contains_key(key).map_err(Error::Fjall);
                    resp_tx.send(result).is_err()
                }
                Action::Len { table, resp_tx } => {
                    let result = table.len().map_err(Error::Fjall);
                    resp_tx.send(result).is_err()
                }
                Action::Remove {
                    table,
                    key,
                    resp_tx,
                } => {
                    let result = table.remove(key).map_err(Error::Fjall);
                    resp_tx.send(result).is_err()
                }
            };
            if err {
                tracing::warn!("Failed to send response for action");
            }
        }
    });
    tx
}

fn handle_delete_table(ks: Option<&mut Keyspace>, table: PartitionHandle) -> Result<(), Error> {
    let ks = ks.ok_or(Error::StoreNotInitialized)?;
    ks.delete_partition(table)?;
    Ok(())
}

fn handle_create_table(
    ks: Option<&mut Keyspace>,
    name: &str,
) -> Result<(PartitionHandle, bool), Error> {
    let ks = ks.ok_or(Error::StoreNotInitialized)?;
    let new = !ks.partition_exists(name);
    let items = ks.open_partition(name, PartitionCreateOptions::default())?;
    Ok((items, new))
}

pub(crate) async fn create_database(
    qualifier: &str,
    organization: &str,
    application: &str,
) -> Result<Database, Error> {
    let mut tx = start_worker();
    let (resp_tx, resp_rx) = oneshot::channel();
    tx.send(Action::CreateDb {
        qualifier: qualifier.to_string(),
        organization: organization.to_string(),
        application: application.to_string(),
        resp_tx,
    })
    .await?;
    resp_rx.await??;
    Ok(Database(tx))
}

pub async fn create_table<'a, K: Key, V: Value>(
    store: &'a UniStore,
    name: &str,
    replace_if_incomatible: bool,
) -> Result<UniTable<'a, K, V>, crate::Error> {
    let (mut table, new) = store.db.create_table(name).await?;
    let empty = new || store.db.is_table_empty(table.clone()).await?;
    if new || empty {
        // If the table is new or we are not replacing, return the table
        return Ok(UniTable {
            store,
            name: name.to_string(),
            table,
            phantom: std::marker::PhantomData,
        });
    }
    let mut replace = false;
    if let Some((key, val)) = store.db.first_key_value(table.clone()).await? {
        // If the table is not empty, check if the types match
        if let Err(e) = K::from_bytes(&key) {
            if replace_if_incomatible {
                // If we are replacing, we can ignore the type mismatch
                tracing::warn!("Replacing table {} due to key type mismatch: {}", name, e);
                replace = true;
            } else {
                return Err(e);
            }
        }
        if let Err(e) = rmp_serde::from_slice::<V>(&val) {
            if replace_if_incomatible {
                // If we are replacing, we can ignore the type mismatch
                tracing::warn!("Replacing table {} due to value type mismatch: {}", name, e);
                replace = true;
            } else {
                return Err(crate::Error::ValueTypeMismatch(e.to_string()));
            }
        }
    }
    if replace {
        store.db.delete_table(table).await?;
        (table, _) = store.db.create_table(name).await?;
    }

    // check if the types match
    Ok(UniTable {
        store,
        name: name.to_string(),
        table,
        phantom: std::marker::PhantomData,
    })
}

pub async fn insert<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
    value: impl AsValue<V>,
) -> Result<(), Error> {
    let key = key.as_key().as_bytes().into();
    let value = rmp_serde::to_vec(&value)?;
    table
        .store
        .db
        .insert(table.table.clone(), key, value.into())
        .await?;
    Ok(())
}

pub async fn contains<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<bool, Error> {
    let key = key.as_key().as_bytes().into();
    let contains = table.store.db.contains(table.table.clone(), key).await?;
    Ok(contains)
}

pub async fn get<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<Option<V>, Error> {
    let key = key.as_key().as_bytes().into();
    let value = table.store.db.get(table.table.clone(), key).await?;
    match value {
        Some(value) => {
            let value: V = rmp_serde::from_slice(&value)?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

pub async fn remove<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    key: impl AsKey<K>,
) -> Result<(), Error> {
    let key = key.as_key().as_bytes().into();
    table.store.db.remove(table.table.clone(), key).await
}

pub async fn len<K: Key, V: Value>(table: &UniTable<'_, K, V>) -> Result<usize, Error> {
    let empty = table.store.db.is_table_empty(table.table.clone()).await?;
    if empty {
        return Ok(0);
    }
    table.store.db.len(table.table.clone()).await
}

pub async fn is_empty<K: Key, V: Value>(table: &UniTable<'_, K, V>) -> Result<bool, Error> {
    table.store.db.is_table_empty(table.table.clone()).await
}

pub async fn get_prefix<K: Key, V: Value>(
    table: &UniTable<'_, K, V>,
    prefix: impl AsKey<K>,
) -> Result<Vec<(K, V)>, crate::Error> {
    // TODO: use worker thread
    futures::future::ready(()).await;
    let prefix = prefix.as_key().as_bytes();
    let table = table.table.clone();

    let items = table.prefix(prefix);
    let mapped = items
        .map(|i| -> Result<(K, V), crate::Error> {
            let (k, v) = i.map_err(|e| crate::Error::Native(Error::Fjall(e)))?;
            let key = K::from_bytes(&k)?;
            let value = rmp_serde::from_slice::<V>(&v)
                .map_err(|e| crate::Error::Native(Error::RmpDecode(e)))?;
            Ok((key, value))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(mapped)
}

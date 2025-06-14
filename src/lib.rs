#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

use std::marker::PhantomData;

pub use async_std::sync::Mutex;
use serde::{Serialize, de::DeserializeOwned};

pub trait Key: Serialize + DeserializeOwned {}
impl<T: Serialize + DeserializeOwned> Key for T {}

pub trait Value: Serialize + DeserializeOwned {}
impl<T: Serialize + DeserializeOwned> Value for T {}

pub struct UniStore {
    #[cfg(target_arch = "wasm32")]
    db: wasm::Database,
    #[cfg(not(target_arch = "wasm32"))]
    db: native::Database,
    name: String,
}
impl std::fmt::Debug for UniStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UniStore")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

pub struct UniTable<'a, K: Key, V: Value> {
    store: &'a UniStore,
    name: String,
    #[cfg(not(target_arch = "wasm32"))]
    table: native::Table,
    phantom: PhantomData<(K, V)>,
}
impl<K: Key, V: Value> std::fmt::Debug for UniTable<'_, K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UniTable")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[cfg(target_arch = "wasm32")]
    Wasm(#[from] wasm::Error),
    #[cfg(not(target_arch = "wasm32"))]
    Native(#[from] native::Error),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UniStore Error: {self:?}")
    }
}

impl UniStore {
    pub async fn new(name: &str) -> Result<Self, Error> {
        #[cfg(target_arch = "wasm32")]
        let db = wasm::create_database(name).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let db = native::create_database(name).await?;
        Ok(UniStore {
            db,
            name: name.to_string(),
        })
    }

    pub async fn create_table<K: Key, V: Value>(
        &self,
        name: &str,
        replace_if_incompatible: bool,
    ) -> Result<UniTable<K, V>, Error> {
        #[cfg(target_arch = "wasm32")]
        let table = wasm::create_table(self, name, replace_if_incompatible).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let table = native::create_table(self, name, replace_if_incompatible).await?;
        Ok(table)
    }
}

impl<K: Key, V: Value> UniTable<'_, K, V> {
    pub async fn insert(&self, key: K, value: V) -> Result<(), Error> {
        #[cfg(target_arch = "wasm32")]
        wasm::insert(self, key, value).await?;
        #[cfg(not(target_arch = "wasm32"))]
        native::insert(self, key, value).await?;
        Ok(())
    }

    pub async fn contains(&self, key: K) -> Result<bool, Error> {
        #[cfg(target_arch = "wasm32")]
        let exists = wasm::contains(self, key).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let exists = native::contains(self, key).await?;
        Ok(exists)
    }

    pub async fn get(&self, key: K) -> Result<Option<V>, Error> {
        #[cfg(target_arch = "wasm32")]
        let value = wasm::get(self, key).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let value = native::get(self, key).await?;
        Ok(value)
    }
}

#[macro_export]
macro_rules! static_table {
    ($fn_name:ident, $name:literal, $key:ty, $val:ty, $get_store: path) => {
        async fn $fn_name() -> &'static $crate::UniTable<'static, $key, $val> {
            use $crate::Mutex;
            static TABLE: std::sync::OnceLock<$crate::UniTable<'static, $key, $val>> =
                std::sync::OnceLock::new();
            static INITIALIZING: Mutex<()> = Mutex::new(());

            if let Some(table) = TABLE.get() {
                return table;
            }
            let _lock = INITIALIZING.lock().await;
            if let Some(table) = TABLE.get() {
                return table;
            }
            let store = get_store().await;
            let table = store
                .create_table::<$key, $val>($name, true)
                .await
                .expect("Failed to create table");
            TABLE.set(table).expect("Failed to set table");
            TABLE.get().unwrap()
        }
    };
}

#[macro_export]
macro_rules! static_store {
    ($fn_name:ident, $name:literal) => {
        async fn $fn_name() -> &'static $crate::UniStore {
            use $crate::Mutex;
            static STORE: std::sync::OnceLock<$crate::UniStore> = std::sync::OnceLock::new();
            static INITIALIZING: Mutex<()> = Mutex::new(());

            if let Some(store) = STORE.get() {
                return store;
            }
            let _lock = INITIALIZING.lock().await;
            if let Some(store) = STORE.get() {
                return store;
            }
            let store = UniStore::new($name).await.expect("Failed to create store");
            STORE.set(store).expect("Failed to set store");
            STORE.get().unwrap()
        }
    };
}

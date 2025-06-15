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

pub trait AsKey<K: Key>: Serialize {}
impl<K: Key> AsKey<K> for K {}
impl<K: Key> AsKey<K> for &K {}
impl AsKey<String> for &str {}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[cfg(target_arch = "wasm32")]
    #[error("Error in WebAssembly implementation: {0}")]
    Wasm(#[from] wasm::Error),
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Error in native implementation: {0}")]
    Native(#[from] native::Error),
}

impl UniStore {
    pub async fn new(
        qualifier: &str,
        organization: &str,
        application: &str,
    ) -> Result<Self, Error> {
        #[cfg(target_arch = "wasm32")]
        let db = wasm::create_database(application).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let db = native::create_database(qualifier, organization, application).await?;
        let name = format!("{qualifier}.{organization}.{application}");
        Ok(UniStore { db, name })
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
    pub async fn insert(&self, key: impl AsKey<K>, value: V) -> Result<(), Error> {
        #[cfg(target_arch = "wasm32")]
        wasm::insert(self, key, value).await?;
        #[cfg(not(target_arch = "wasm32"))]
        native::insert(self, key, value).await?;
        Ok(())
    }

    pub async fn contains(&self, key: impl AsKey<K>) -> Result<bool, Error> {
        #[cfg(target_arch = "wasm32")]
        let exists = wasm::contains(self, key).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let exists = native::contains(self, key).await?;
        Ok(exists)
    }

    pub async fn get(&self, key: impl AsKey<K>) -> Result<Option<V>, Error> {
        #[cfg(target_arch = "wasm32")]
        let value = wasm::get(self, key).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let value = native::get(self, key).await?;
        Ok(value)
    }

    pub async fn remove(&self, key: impl AsKey<K>) -> Result<(), Error> {
        #[cfg(target_arch = "wasm32")]
        wasm::remove(self, key).await?;
        #[cfg(not(target_arch = "wasm32"))]
        native::remove(self, key).await?;
        Ok(())
    }

    pub async fn len(&self) -> Result<usize, Error> {
        #[cfg(target_arch = "wasm32")]
        let count = wasm::len(self).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let count = native::len(self).await?;
        Ok(count)
    }

    pub async fn is_empty(&self) -> Result<bool, Error> {
        #[cfg(target_arch = "wasm32")]
        let empty = wasm::is_empty(self).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let empty = native::is_empty(self).await?;
        Ok(empty)
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
    ($fn_name:ident, $qualifier:literal, $organization:literal, $application:literal) => {
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
            let store = UniStore::new($qualifier, $organization, $application)
                .await
                .expect("Failed to create store");
            STORE.set(store).expect("Failed to set store");
            STORE.get().unwrap()
        }
    };
}

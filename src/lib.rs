#[cfg(not(target_arch = "wasm32"))]
mod native;
// #[cfg(target_arch = "wasm32")]
mod wasm;

use std::{marker::PhantomData, sync::OnceLock};

use serde::{Serialize, de::DeserializeOwned};

pub struct UniStore<K, V> {
    // #[cfg(target_arch = "wasm32")]
    db: wasm::Database,
    name: String,
    phantom: PhantomData<(K, V)>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[cfg(target_arch = "wasm32")]
    Wasm(#[from] wasm::Error),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UniStore Error: {self:?}")
    }
}

impl<K, V> UniStore<K, V>
where
    K: Serialize,
    V: Serialize + DeserializeOwned,
{
    pub async fn new(name: &str) -> Result<Self, Error> {
        #[cfg(target_arch = "wasm32")]
        let db = wasm::create_database(name).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let db = native::create_database(name).await?;
        Ok(UniStore {
            db,
            name: name.to_string(),
            phantom: PhantomData,
        })
    }

    pub async fn insert(&self, key: K, value: V) -> Result<(), Error> {
        #[cfg(target_arch = "wasm32")]
        wasm::insert(self, key, value).await?;
        Ok(())
    }

    pub async fn contains(&self, key: K) -> Result<bool, Error> {
        #[cfg(target_arch = "wasm32")]
        let exists = wasm::contains(self, key).await?;
        Ok(exists)
    }

    pub async fn get(&self, key: K) -> Result<Option<V>, Error> {
        #[cfg(target_arch = "wasm32")]
        let value = wasm::get(self, key).await?;
        Ok(value)
    }
}

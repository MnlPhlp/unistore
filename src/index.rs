use crate::{AsKey, Key, UniTable, Value};

pub struct UniIndex<'a, I: Key, K: Key, V: Value> {
    pub table: &'a UniTable<'a, K, V>,
    pub index: UniTable<'a, String, ()>,
    pub index_rev: UniTable<'a, String, String>,
    phantom: std::marker::PhantomData<I>,
}

impl<I: Key, K: Key, V: Value> std::fmt::Debug for UniIndex<'_, I, K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UniIndex")
            .field("table", &self.table.name)
            .field("index", &self.index.name)
            .finish()
    }
}

impl<I: Key, K: Key + Clone, V: Value> UniIndex<'_, I, K, V> {
    pub async fn get(&self, key: impl AsKey<I>) -> Result<Vec<(K, V)>, crate::Error> {
        let index_entries = self.index.get_prefix(key.as_key().to_key_string()).await?;
        let mut results = Vec::new();
        for (index_key, ()) in index_entries {
            let (_, key) = index_key
                .split_once('\0')
                .expect("Index key should contain a separator");
            let key = K::from_key_string(key)?;
            if let Some(value) = self.table.get(key.clone()).await? {
                results.push((key, value));
            }
        }
        Ok(results)
    }

    pub async fn get_first(&self, key: impl AsKey<I>) -> Result<Option<(K, V)>, crate::Error> {
        let index_entries = self.index.get_prefix(key.as_key().to_key_string()).await?;
        if index_entries.is_empty() {
            return Ok(None);
        }
        let (index_key, ()) = index_entries.into_iter().next().unwrap();
        let (_, key) = index_key
            .split_once('\0')
            .expect("Index key should contain a separator");
        let key = K::from_key_string(key)?;
        if let Some(value) = self.table.get(key.clone()).await? {
            return Ok(Some((key, value)));
        }
        Ok(None)
    }

    pub async fn insert(
        &self,
        value: impl AsKey<I>,
        key: impl AsKey<K>,
    ) -> Result<(), crate::Error> {
        let key_str = key.as_key().to_key_string();
        let value_str = value.as_key().to_key_string();
        if let Some(existing) = self.index_rev.get(key_str.as_str()).await? {
            self.index.remove(existing).await?;
        }
        let index_key = format!("{value_str}\0{key_str}");
        self.index.insert(index_key.clone(), ()).await?;
        self.index_rev.insert(key_str, index_key).await?;
        Ok(())
    }
}

impl<K: Key, V: Value> UniTable<'_, K, V> {
    pub async fn create_index<I: Key>(
        &self,
        index: &'static str,
    ) -> Result<UniIndex<'_, I, K, V>, crate::Error> {
        let index_table = self
            .store
            .create_table(&format!("{}_index_{index}", self.name), false)
            .await?;
        let rev_index_table = self
            .store
            .create_table(&format!("{}_index_{index}_rev", self.name), false)
            .await?;
        Ok(UniIndex {
            table: self,
            index: index_table,
            index_rev: rev_index_table,
            phantom: std::marker::PhantomData,
        })
    }
}

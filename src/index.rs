use crate::{AsKey, Key, UniTable, Value};

pub struct UniIndex<'a, I: Key, K: Key, V: Value> {
    pub table: &'a UniTable<'a, K, V>,
    pub index: UniTable<'a, String, ()>,
    phantom: std::marker::PhantomData<I>,
}

impl<I: Key, K: Key + Clone, V: Value> UniIndex<'_, I, K, V> {
    pub async fn get(&self, key: impl AsKey<I>) -> Result<Vec<(K, V)>, crate::Error> {
        let index_entries = self.index.get_prefix(key.as_key().to_key_string()).await?;
        let mut results = Vec::new();
        for (index_key, _) in index_entries {
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

    pub async fn insert(
        &self,
        value: impl AsKey<I>,
        key: impl AsKey<K>,
    ) -> Result<(), crate::Error> {
        let key = key.as_key();
        let index_key = format!(
            "{}\0{}",
            value.as_key().to_key_string(),
            key.as_key().to_key_string()
        );
        self.index.insert(index_key, ()).await?;
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
        Ok(UniIndex {
            table: self,
            index: index_table,
            phantom: std::marker::PhantomData,
        })
    }
}

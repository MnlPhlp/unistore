use crate::{AsKey, Error, Key, Value, index::UniIndex};
pub trait UniStoreItem: Value + 'static {
    type Key: Key + 'static;

    fn table() -> impl Future<Output = &'static crate::UniTable<'static, Self::Key, Self>>;
    fn unistore_key(&self) -> Self::Key;

    #[must_use]
    fn index_table(
        index: &'static str,
    ) -> impl Future<Output = Result<&'static UniIndex<'static, String, Self::Key, Self>, Error>>
    {
        futures::future::ready(Err(Error::MissingIndex(index)))
    }

    /// This function is called to insert indices for the item.
    /// It is a no-op by default, but can be overridden in the implementation.
    /// It is called by default when the item is inserted into the table using the traits `insert` method.
    fn insert_indices(&self) -> impl Future<Output = Result<(), Error>> {
        futures::future::ready(Ok(()))
    }

    fn get(key: impl AsKey<Self::Key>) -> impl Future<Output = Result<Option<Self>, crate::Error>> {
        async move {
            let table = Self::table().await;
            table.get(key).await
        }
    }
    fn get_by_index<I: Key>(
        index: &'static str,
        key: impl AsKey<I>,
    ) -> impl Future<Output = Result<Vec<(Self::Key, Self)>, crate::Error>> {
        async move {
            let table = Self::index_table(index).await?;
            table.get(key.as_key().to_key_string()).await
        }
    }
    fn get_first_by_index(
        index: &'static str,
        key: impl AsKey<String>,
    ) -> impl Future<Output = Result<Option<(Self::Key, Self)>, crate::Error>> {
        async move {
            let table = Self::index_table(index).await?;
            table.get_first(key).await
        }
    }
    fn save(&self) -> impl Future<Output = Result<(), crate::Error>> {
        let key = self.unistore_key();
        async move {
            self.insert_indices().await?;
            let table = Self::table().await;
            table.insert(key, self).await
        }
    }
    fn contains(key: impl AsKey<Self::Key>) -> impl Future<Output = Result<bool, crate::Error>> {
        async move {
            let table = Self::table().await;
            table.contains(key).await
        }
    }
    fn remove(key: impl AsKey<Self::Key>) -> impl Future<Output = Result<(), crate::Error>> {
        async move {
            let table = Self::table().await;
            table.remove(key).await
        }
    }
}

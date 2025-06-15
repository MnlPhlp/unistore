use crate::{AsKey, Key, Value};
pub trait UniStoreItem: Value + 'static {
    type Key: Key + 'static;

    fn table() -> impl Future<Output = &'static crate::UniTable<'static, Self::Key, Self>>;
    fn unistore_key(&self) -> Self::Key;

    fn get(key: impl AsKey<Self::Key>) -> impl Future<Output = Result<Option<Self>, crate::Error>> {
        async move {
            let table = Self::table().await;
            table.get(key).await
        }
    }
    fn insert(&self) -> impl Future<Output = Result<(), crate::Error>> {
        let key = self.unistore_key();
        async move {
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

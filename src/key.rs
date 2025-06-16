use crate::Error;

pub trait Key: Sized + Clone {
    fn as_bytes(self) -> Vec<u8>;
    fn to_key_string(self) -> String;
    fn from_bytes(slice: &[u8]) -> Result<Self, Error>;
    fn from_key_string(s: &str) -> Result<Self, Error>;
}
impl Key for String {
    fn as_bytes(self) -> Vec<u8> {
        self.into_bytes()
    }

    fn to_key_string(self) -> String {
        self
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(String::from_utf8(bytes.to_vec()).map_err(|e| Error::KeyTypeMismatch(e.to_string()))?)
    }

    fn from_key_string(s: &str) -> Result<Self, Error> {
        Ok(s.to_string())
    }
}
macro_rules! num_key {
    ($t:ty) => {
        impl Key for $t {
            fn as_bytes(self) -> Vec<u8> {
                self.to_be_bytes().to_vec()
            }

            fn to_key_string(self) -> String {
                self.to_string()
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
                let bytes = bytes.try_into().map_err(|_| {
                    Error::KeyTypeMismatch(format!("Invalid key length for {}", stringify!($t)))
                })?;
                Ok(Self::from_be_bytes(bytes))
            }

            fn from_key_string(s: &str) -> Result<Self, Error> {
                s.parse::<Self>()
                    .map_err(|e| Error::KeyTypeMismatch(e.to_string()))
            }
        }
    };
}
num_key!(u8);
num_key!(u16);
num_key!(u32);
num_key!(u64);
num_key!(i8);
num_key!(i16);
num_key!(i32);
num_key!(i64);

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::de::{Error, Visitor};
use serde::{Deserialize, Serialize};

struct StringOrNumber(u64);

impl Serialize for StringOrNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0 <= 0x1fffffffffffffu64 || !serializer.is_human_readable() {
            serializer.serialize_u64(self.0)
        } else {
            serializer.serialize_str(&self.0.to_string())
        }
    }
}

impl<'de> Deserialize<'de> for StringOrNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value<'a> {
            String(#[serde(borrow)] Cow<'a, str>),
            Number(u64),
        }

        match Value::deserialize(deserializer)? {
            Value::String(str) => u64::from_str(str.as_ref())
                .map(Self)
                .map_err(|_| D::Error::custom("Invalid number")),
            Value::Number(value) => Ok(Self(value)),
        }
    }
}

pub mod string {
    use super::*;

    pub fn serialize<S>(value: &dyn std::fmt::Display, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        BorrowedStr::deserialize(deserializer)
            .and_then(|data| T::from_str(&data.0).map_err(D::Error::custom))
    }
}

pub mod serde_optional_u64 {
    use super::*;
    use serde::Serialize;

    pub fn serialize<S>(data: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        data.map(StringOrNumber).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Option::<StringOrNumber>::deserialize(deserializer)?.map(|StringOrNumber(x)| x))
    }
}


pub mod serde_hex_array {
    use super::*;

    pub fn serialize<S>(data: &dyn AsRef<[u8]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde_bytes::serialize(data, serializer)
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = serde_bytes::deserialize(deserializer)?;
        data.try_into()
            .map_err(|_| D::Error::custom(format!("Invalid array length, expected: {N}")))
    }
}

pub mod serde_bytes {
    use std::fmt;

    use serde::de::Unexpected;

    use super::*;

    pub fn serialize<S>(data: &dyn AsRef<[u8]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&hex::encode(data.as_ref()))
        } else {
            serializer.serialize_bytes(data.as_ref())
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HexVisitor;

        impl<'de> Visitor<'de> for HexVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("hex-encoded byte array")
            }

            fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
                hex::decode(value).map_err(|_| E::invalid_type(Unexpected::Str(value), &self))
            }

            // See the `deserializing_flattened_field` test for an example why this is needed.
            fn visit_bytes<E: Error>(self, value: &[u8]) -> Result<Self::Value, E> {
                Ok(value.to_vec())
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(HexVisitor)
        } else {
            deserializer.deserialize_bytes(BytesVisitor)
        }
    }
}

struct BytesVisitor;

impl<'de> Visitor<'de> for BytesVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("byte array")
    }

    fn visit_bytes<E: Error>(self, value: &[u8]) -> Result<Self::Value, E> {
        Ok(value.to_vec())
    }
}

#[derive(Deserialize)]
#[repr(transparent)]
pub struct BorrowedStr<'a>(#[serde(borrow)] pub Cow<'a, str>);

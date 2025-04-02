use std::borrow::Cow;
use std::str::FromStr;

use serde::de::Error;
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

#[derive(Deserialize)]
#[repr(transparent)]
pub struct BorrowedStr<'a>(#[serde(borrow)] pub Cow<'a, str>);

use std::borrow::Cow;
use std::str::FromStr;

use serde::de::Error;
use serde::Deserialize;

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

#[derive(Deserialize)]
#[repr(transparent)]
pub struct BorrowedStr<'a>(#[serde(borrow)] pub Cow<'a, str>);

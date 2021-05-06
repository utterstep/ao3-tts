use std::{borrow::Cow, fmt, ops::Deref};

use serde::{
    de::{self, Deserializer, Visitor},
    ser::Serializer,
    Deserialize, Serialize,
};

/// Custom struct to avoid extra allocations while deserializing
/// saved data
///
/// Based on
/// [this](https://github.com/serde-rs/serde/issues/1852#issuecomment-577460985)
/// GitHub commit
#[derive(Debug)]
pub(crate) struct CowBytes<'a>(Cow<'a, [u8]>);

impl<'a> Deref for CowBytes<'a> {
    type Target = Cow<'a, [u8]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> From<Vec<u8>> for CowBytes<'a> {
    fn from(value: Vec<u8>) -> Self {
        Self(Cow::Owned(value))
    }
}

impl<'a> Serialize for CowBytes<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(self.0.as_ref())
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for CowBytes<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(CowBytesVisitor)
    }
}

/// Does the heavy lifting of visiting borrowed bytes
struct CowBytesVisitor;

impl<'de> Visitor<'de> for CowBytesVisitor {
    type Value = CowBytes<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes")
    }

    // A slice that currently only lives in a temporary buffer - we need a copy
    // (Example: serde is reading from a BufRead)
    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(CowBytes(Cow::Owned(value.to_owned())))
    }

    // Borrowed directly from the input slice, which has lifetime 'de
    // The input must outlive the resulting Cow.
    fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(CowBytes(Cow::Borrowed(value)))
    }

    // A string that currently only lives in a temporary buffer -- we need a copy
    // (Example: serde is reading from a BufRead)
    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(CowBytes(Cow::Owned(value)))
    }
}

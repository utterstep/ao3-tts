use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod cowbytes;

pub(crate) use cowbytes::CowBytes;

#[derive(Debug, Deserialize, Serialize, Default)]
pub(crate) struct ProcessedData<'a>(#[serde(borrow)] pub BTreeMap<&'a str, CowBytes<'a>>);

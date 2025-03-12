//! Tinfoil "Index" JSON response data types.
//! Tinfoil expects to read a json "index", which essentially just acts as a response format
//! and lists all the files available for serving to the client.


use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;


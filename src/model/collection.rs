use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub struct Collection(pub String);

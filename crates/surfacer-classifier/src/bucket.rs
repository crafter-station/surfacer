use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassifierBucket {
    FormSessionLegacy,
    RestModernSpa,
    GraphqlIntrospectable,
    AxOnly,
    HtmlRendered,
    Hostile,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

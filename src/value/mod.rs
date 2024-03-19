pub(crate) mod input;
pub(crate) mod output;

/// [`serde_json::Value`] from [`serde_json`].
pub type Value = serde_json::Value;
/// [`serde_json::Number`] from [`serde_json`].
pub type Number = serde_json::Number;
/// [`serde_json::Map`] from [`serde_json`].
pub type Map<K, V> = serde_json::Map<K, V>;
/// [`serde_json::from_value`] from [`serde_json`].
pub use serde_json::from_value;
/// [`serde_json::to_value`] from [`serde_json`].
pub use serde_json::to_value;

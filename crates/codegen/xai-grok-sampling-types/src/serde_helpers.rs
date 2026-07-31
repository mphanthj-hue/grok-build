use serde::{Deserialize, Deserializer};

pub fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.filter(|s| !s.is_empty()))
}

/// Deserialize `Option<Option<T>>`: absent (`None`) leaves, `null` (`Some(None)`)
/// clears, a value sets. Requires `#[serde(default, deserialize_with = "…")]`.
pub fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// Lenient string deserialization: accepts a JSON string, a number (e.g. a
/// numeric model id sent as a bare integer), or a boolean; falls back to ""
/// for any other shape (null, object, array).
pub fn string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    })
}

/// Lenient u64 deserialization: accepts a JSON number or a numeric string
/// (e.g. `"created": "1720000000"`); falls back to 0 for any other shape.
pub fn u64_or_default<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
        serde_json::Value::String(s) => s.trim().parse::<u64>().unwrap_or(0),
        _ => 0,
    })
}

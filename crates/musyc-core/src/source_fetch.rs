//! 通用 JSON 点路径字段提取（generic-json 源用）。
//!
//! 点路径支持方括号数字下标：`results[0].tracks` / `items.0.name` / `data["list"]`。

use serde_json::Value;

/// 取 JSON 里按点路径定位的值。空路径返回根本身。
pub fn get_path<'v>(root: &'v Value, path: &str) -> Option<&'v Value> {
    let path = path.trim();
    if path.is_empty() {
        return Some(root);
    }
    let mut current = root;
    for part in split_path(path) {
        current = match current {
            Value::Object(map) => map.get(&part)?,
            Value::Array(items) => items.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

/// 把 `a[0].b` 规范化后拆成 `["a", "0", "b"]`。
fn split_path(path: &str) -> Vec<String> {
    let normalized = path.replace("][", ".").replace('[', ".").replace(']', "");
    normalized
        .split('.')
        .map(|seg| seg.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|seg| !seg.is_empty())
        .collect()
}

/// 把一个 JSON 值（可能是字符串或数字）按毫秒/秒转成 u64。
pub fn to_millis(value: Option<&Value>, unit: crate::sources::DurationUnit) -> u64 {
    let Some(v) = value else { return 0 };
    let raw = match v {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_i64().and_then(|i| u64::try_from(i).ok()))
            .or_else(|| n.as_f64().map(|f| f.max(0.0) as u64))
            .unwrap_or(0),
        Value::String(s) => s.trim().parse::<u64>().unwrap_or(0),
        _ => 0,
    };
    match unit {
        crate::sources::DurationUnit::Milliseconds => raw,
        crate::sources::DurationUnit::Seconds => raw.saturating_mul(1000),
    }
}

/// 把 JSON 值转成字符串（数字也接受）。
pub fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// 把 JSON 值按数组处理：数组则原样取；对象则尝试常见 list 字段；否则返回空。
pub fn as_array(v: &Value, keys: &[&str]) -> Vec<Value> {
    if let Some(arr) = v.as_array() {
        return arr.clone();
    }
    for key in keys {
        if let Some(Value::Array(arr)) = v.get(*key) {
            return arr.clone();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn split_path_supports_brackets() {
        assert_eq!(
            split_path("results[0].tracks"),
            vec!["results", "0", "tracks"]
        );
        assert_eq!(split_path("items.0.name"), vec!["items", "0", "name"]);
        assert_eq!(split_path(""), Vec::<String>::new());
    }

    #[test]
    fn get_path_traces_nested_arrays() {
        let v = json!({"results": [{"tracks": [1, 2, 3]}]});
        assert_eq!(get_path(&v, "results[0].tracks"), Some(&json!([1, 2, 3])));
    }
}

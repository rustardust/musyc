use super::*;

pub(super) fn pick_value<'v>(item: &'v Value, path: &str) -> Option<&'v Value> {
    if path.trim().is_empty() {
        return None;
    }
    get_path(item, path)
}

pub(super) fn pick_string(item: &Value, path: &str) -> Option<String> {
    pick_value(item, path).and_then(as_string)
}

pub(super) fn artists_from(item: &Value, path: &str) -> Vec<String> {
    let v = match pick_value(item, path) {
        Some(v) => v,
        None => match item
            .get("artists")
            .or_else(|| item.get("artist"))
            .or_else(|| item.get("artist_name"))
        {
            Some(v) => v,
            None => return Vec::new(),
        },
    };
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(arr) => arr
            .iter()
            .filter_map(|x| {
                x.as_str()
                    .map(str::to_string)
                    .or_else(|| x.get("name").and_then(as_string))
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) async fn get_json(
    http: &reqwest::Client,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<Value> {
    let mut req = http.get(url).timeout(LIST_TIMEOUT).header("User-Agent", UA);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| Error::Network(format!("请求 {url} 失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::Network(format!("{url} 返回 HTTP {}", resp.status())));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| Error::Network(format!("读取响应失败: {e}")))?;
    serde_json::from_str(&text).map_err(|e| Error::Api(format!("响应不是合法 JSON: {e}")))
}

pub(super) async fn get_text(
    http: &reqwest::Client,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<String> {
    let mut req = http.get(url).timeout(LIST_TIMEOUT).header("User-Agent", UA);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| Error::Network(format!("请求 {url} 失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::Network(format!("{url} 返回 HTTP {}", resp.status())));
    }
    resp.text()
        .await
        .map_err(|e| Error::Network(format!("读取响应失败: {e}")))
}

pub(super) fn fma_headers() -> &'static [(&'static str, &'static str)] {
    &[("Accept", "text/html")]
}

pub(super) fn source_generic_headers(_source: &Source) -> &'static [(&'static str, &'static str)] {
    &[]
}

pub(super) fn urlencode(s: &str) -> String {
    // reqwest 0.13 带 query(&[("k","v")]) 专用方法；这里只拼客户端 id / 数字 id，不需要完整转义。
    s.replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
}

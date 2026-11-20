use std::time::Duration;

use musyc_core::config::ApiConfig;
use musyc_core::model::{Playlist, Quality, Song, SourceKind};
use musyc_core::{Error, Result};
use serde_json::Value;

use crate::dto;

#[derive(Debug, Clone, Default)]
pub struct SongUrl {
    pub url: String,
    pub br: u64,
    pub size: u64,
    pub format: String,
    /// 官方只返回 30 秒试听（`freeTrialInfo` 非空）。
    pub trial: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RawLyric {
    pub lrc: String,
    pub tlyric: String,
}

/// 街声单曲详情（播放前用于补时长、专辑与歌词）。
#[derive(Debug, Clone, Default)]
pub struct StreetVoiceDetail {
    pub duration_ms: u64,
    pub album: String,
    pub cover_url: Option<String>,
    pub lyrics: String,
    pub lyrics_is_lrc: bool,
}

mod indie;
mod netease;
mod streetvoice;

/// 对 netease-cloud-music-api 的薄封装。
pub struct ApiClient {
    http: reqwest::Client,
    base: String,
    real_ip: Option<String>,
}

impl ApiClient {
    pub fn new(cfg: &ApiConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs.clamp(3, 60)))
            .user_agent(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/131.0 Safari/537.36 musyc/0.1",
            )
            .build()
            .map_err(|e| Error::Network(format!("构建 HTTP 客户端失败: {e}")))?;
        let real_ip = if cfg.real_ip.trim().is_empty() {
            None
        } else {
            Some(cfg.real_ip.trim().to_string())
        };
        Ok(Self {
            http,
            base: cfg.base_url.trim_end_matches('/').to_string(),
            real_ip,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// 启动时的连通性检查。
    pub async fn ping(&self) -> Result<()> {
        let v = self
            .get("search/hot/detail", vec![])
            .await
            .map_err(|e| Error::Network(format!("无法连接 {}: {e}", self.base)))?;
        if v.get("code").and_then(Value::as_i64).unwrap_or(200) == 200 {
            Ok(())
        } else {
            Err(Error::Api(format!("{} 返回异常", self.base)))
        }
    }

    async fn get(&self, path: &str, params: Vec<(&str, String)>) -> Result<Value> {
        self.get_inner(path, params, false).await
    }

    /// 本机 musyc 路由：绕开 apicache，避免旧目录/同名歌曲串数据。
    async fn get_local(&self, path: &str, params: Vec<(&str, String)>) -> Result<Value> {
        self.get_inner(path, params, true).await
    }

    async fn get_inner(
        &self,
        path: &str,
        params: Vec<(&str, String)>,
        bypass_cache: bool,
    ) -> Result<Value> {
        let url = format!("{}/{}", self.base, path.trim_start_matches('/'));
        let mut last_err = String::new();
        // 上游（网易）偶发 ECONNRESET 与 -460 风控，多试几次通常就能过。
        for attempt in 0..4 {
            let mut req = self.http.get(&url).query(&params);
            if bypass_cache {
                req = req.header("x-apicache-bypass", "1");
            }
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp
                        .text()
                        .await
                        .map_err(|e| Error::Network(format!("读取响应失败: {e}")))?;
                    if !status.is_success() {
                        last_err = format!(
                            "HTTP {status}: {}",
                            text.chars().take(160).collect::<String>()
                        );
                    } else {
                        let value: Value = serde_json::from_str(&text).map_err(|e| {
                            Error::Api(format!(
                                "响应不是合法 JSON: {e}: {}",
                                text.chars().take(120).collect::<String>()
                            ))
                        })?;
                        match value.get("code").and_then(Value::as_i64) {
                            Some(301) => {
                                return Err(Error::Api("需要登录后才能使用该功能".into()));
                            }
                            Some(code) if !(0..500).contains(&code) => {
                                // -460 是风控，502 是上游连接被重置：都不立即失败。
                                let msg = value
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .unwrap_or("接口返回错误");
                                last_err = format!("{msg} (code {code})");
                            }
                            _ => return Ok(value),
                        }
                    }
                }
                Err(e) => last_err = e.to_string(),
            }
            if attempt < 3 {
                tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
            }
        }
        Err(Error::Network(format!("请求 {url} 失败: {last_err}")))
    }

    async fn song_url_v1(&self, id: &str, quality: Quality) -> Result<Option<SongUrl>> {
        let mut params = vec![
            ("id", id.to_string()),
            ("level", quality.level().to_string()),
        ];
        // realIP 只用于播放链接；搜索等接口带上它反而容易被风控拒绝。
        if let Some(ip) = &self.real_ip {
            params.push(("realIP", ip.clone()));
        }
        let v = self.get("song/url/v1", params).await?;
        let Some(item) = v
            .get("data")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
        else {
            return Ok(None);
        };
        let Some(url) = item.get("url").and_then(Value::as_str) else {
            return Ok(None);
        };
        if url.trim().is_empty() {
            return Ok(None);
        }
        let url = if let Some(rest) = url.strip_prefix("http://") {
            format!("https://{rest}")
        } else {
            url.to_string()
        };
        let format = item
            .get("type")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .or_else(|| {
                url.split('?')
                    .next()
                    .and_then(|p| p.rsplit('.').next())
                    .map(|s| s.to_ascii_lowercase())
            })
            .unwrap_or_else(|| "mp3".into());
        let trial = item.get("freeTrialInfo").is_some_and(|v| !v.is_null());
        Ok(Some(SongUrl {
            url,
            br: item.get("br").and_then(Value::as_u64).unwrap_or(0),
            size: item.get("size").and_then(Value::as_u64).unwrap_or(0),
            format,
            trial,
        }))
    }

    /// 旧版播放链路（br 参数），作为 v1 被风控时的兜底。
    async fn song_url_legacy(&self, id: &str) -> Result<Option<SongUrl>> {
        let v = self
            .get(
                "song/url",
                vec![("id", id.to_string()), ("br", "999000".into())],
            )
            .await?;
        let Some(item) = v
            .get("data")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
        else {
            return Ok(None);
        };
        let Some(url) = item.get("url").and_then(Value::as_str) else {
            return Ok(None);
        };
        if url.trim().is_empty() {
            return Ok(None);
        }
        let url = if let Some(rest) = url.strip_prefix("http://") {
            format!("https://{rest}")
        } else {
            url.to_string()
        };
        let format = item
            .get("type")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .or_else(|| {
                url.split('?')
                    .next()
                    .and_then(|p| p.rsplit('.').next())
                    .map(|s| s.to_ascii_lowercase())
            })
            .unwrap_or_else(|| "mp3".into());
        let trial = item.get("freeTrialInfo").is_some_and(|v| !v.is_null());
        Ok(Some(SongUrl {
            url,
            br: item.get("br").and_then(Value::as_u64).unwrap_or(0),
            size: item.get("size").and_then(Value::as_u64).unwrap_or(0),
            format,
            trial,
        }))
    }

    async fn streetvoice_list(&self, kind: &str) -> Result<Vec<Song>> {
        let v = self
            .get_local(&format!("musyc/catalog/streetvoice/{kind}"), vec![])
            .await?;
        Ok(dto::array_of(&v, &["songs"])
            .iter()
            .filter_map(|x| dto::source_song_from(x, SourceKind::StreetVoice))
            .collect())
    }
}

use super::*;

impl App {
    pub(super) fn download_selected(&mut self) {
        // 优先使用光标选中的歌曲；选中行不是歌曲（首页 / 歌单列表）时回退到正在播放的歌。
        let Some(song) = self.selected_song().or_else(|| self.current.clone()) else {
            self.toast("当前没有歌曲");
            return;
        };
        if !song.source.is_online() {
            self.toast("无需下载");
            return;
        }
        if self.downloading.is_some() {
            self.toast("下载中…");
            return;
        }

        self.download_seq += 1;
        let seq = self.download_seq;
        self.downloading = Some((seq, song.name.clone()));
        self.download_progress = None;

        let resolver = self.resolver.clone();
        let tx = self.tx.clone();
        let http = self.http.clone();
        let api = self.api.clone();
        let dir = self.cfg.download.resolved_dir();
        let quality = self.quality;
        let log_dir = self.paths.log_dir.clone();
        // 外部库回退客户端：可能因环境代理导致不可用，但只影响标签补缺，不阻断下载
        let web = fallback::external_client().ok();
        tokio::spawn(async move {
            let source = match resolver.resolve(&song, quality).await {
                Ok(source) => source,
                Err(e) => {
                    let _ = tx.send(Event::DownloadFailed {
                        seq,
                        message: e.to_string(),
                    });
                    return;
                }
            };
            // 先用来源自带的正确标签（雀乐 / 街声 / 网易云元数据），
            // 歌词和封面则在源库里查不到时回退外部库，与标签补全同一套流程。
            let artist = musyc_library::sanitize(&song.artist_text());
            let title = musyc_library::sanitize(&song.name);
            let ext: String = source
                .format
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            let ext = if ext.is_empty() {
                "mp3".to_string()
            } else {
                ext
            };
            let dest = dir.join(format!("{artist} - {title}.{ext}"));
            let tx_progress = tx.clone();
            let mut last_pct = i32::MIN;
            let result =
                musyc_library::download_to(&http, &source.url, &dest, move |done, total| {
                    let pct = total
                        .filter(|t| *t > 0)
                        .map(|t| (done.saturating_mul(100) / t) as i32)
                        .unwrap_or(-1);
                    if pct != last_pct {
                        last_pct = pct;
                        let _ = tx_progress.send(Event::DownloadProgress { seq, done, total });
                    }
                })
                .await;
            match result {
                Ok(outcome) => {
                    let path = outcome.path.clone();
                    let bytes = outcome.bytes;
                    let tagged =
                        Self::tag_downloaded(&api, &http, web.as_ref(), &song, &path, &log_dir)
                            .await;
                    let _ = tx.send(Event::DownloadDone {
                        seq,
                        path,
                        bytes,
                        tagged,
                    });
                }
                Err(e) => {
                    let _ = tx.send(Event::DownloadFailed {
                        seq,
                        message: e.to_string(),
                    });
                }
            }
        });
    }
    /// 下载完成后把标题 / 歌手 / 专辑 / 歌词 / 封面写进文件。
    /// 优先用来源自带的数据，缺失的歌词或封面再按需用外部库补，
    /// 下载完成后把标题 / 歌手 / 专辑 / 歌词 / 封面写进文件。
    /// 优先用来源自带的数据，缺失的歌词或封面再按需用外部库补，
    /// 全部失败不打乱下载流程，只返回一句说明让调用方 toast。
    pub(super) async fn tag_downloaded(
        api: &ApiClient,
        http: &reqwest::Client,
        web: Option<&reqwest::Client>,
        song: &Song,
        path: &Path,
        log_dir: &Path,
    ) -> String {
        let title = song.name.trim().to_string();
        let artists: Vec<String> = song
            .artists
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let album = song.album.trim().to_string();
        let origin = path.to_path_buf();

        // 歌词：先源库，再 LRCLIB（若外部库可用）
        let artist_name = song.artist_text();
        let mut lyric = Self::lyric_for_source(api, http, song).await;
        if lyric.is_none()
            && let Some(web) = web
            && !title.is_empty()
        {
            lyric = fallback::lrclib_lyrics(web, &title, &artist_name, song.duration_ms).await;
        }

        // 封面：先来源自带的 URL（带 Referer），再 MusicBrainz + Cover Art Archive（若外部库可用）
        let mut cover = match song.cover_url.as_deref().filter(|u| !u.trim().is_empty()) {
            Some(url) => match http
                .get(url)
                .header("Referer", "https://music.163.com/")
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => resp
                    .bytes()
                    .await
                    .ok()
                    .map(|b| b.to_vec())
                    .filter(|b| !b.is_empty()),
                _ => None,
            },
            None => None,
        };
        if cover.is_none()
            && let Some(web) = web
            && !title.is_empty()
        {
            cover = fallback::musicbrainz_meta(web, &title, &artist_name, song.duration_ms)
                .await
                .and_then(|meta| meta.cover);
        }

        let title_opt = (!title.is_empty()).then_some(title.as_str());
        let album_opt = (!album.is_empty()).then_some(album.as_str());
        match musyc_library::write_meta_full(
            path,
            title_opt,
            &artists,
            album_opt,
            lyric.as_deref(),
            cover.as_deref(),
        ) {
            Ok(()) => {
                let mut parts = Vec::new();
                if title_opt.is_some() || !artists.is_empty() {
                    parts.push("标题/歌手/专辑");
                }
                if lyric.is_some() {
                    parts.push("歌词");
                }
                if cover.is_some() {
                    parts.push("封面");
                }
                if parts.is_empty() {
                    "标签已写入".to_string()
                } else {
                    format!("标签已写入（{}）", parts.join("、"))
                }
            }
            Err(e) => {
                let reason = e.to_string();
                fallback::log_failure(log_dir, &origin, &title, &reason);
                format!("标签写入失败：{reason}")
            }
        }
    }
}

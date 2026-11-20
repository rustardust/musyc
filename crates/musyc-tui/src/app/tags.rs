use super::*;

impl App {
    pub(super) fn tag_selected(&mut self) {
        if self.view != View::Local || self.search.is_some() {
            self.toast("本地补全标签");
            return;
        }
        if self.tagging {
            self.toast("正在补全中");
            return;
        }
        let Some(index) = self.selected_index() else {
            return;
        };
        let Some(song) = self.local_songs.get(index).cloned() else {
            return;
        };
        let origin = local_origin(&song);
        if !origin.exists() {
            self.toast("文件不存在");
            return;
        }
        let source_dir = self.cfg.local.resolved_dir();
        let output_dir = self.cfg.local.resolved_output_dir();
        if same_dir(&source_dir, &output_dir) {
            self.toast("目录不能相同");
            return;
        }

        self.tag_seq += 1;
        let seq = self.tag_seq;
        self.tagging = true;
        self.toast = None;
        self.tag_progress = Some((0, 1, song.name.clone()));
        let api = self.api.clone();
        let http = self.http.clone();
        let tx = self.tx.clone();
        let log_dir = self.paths.log_dir.clone();
        let web = match fallback::external_client() {
            Ok(client) => client,
            Err(_) => {
                self.tagging = false;
                self.tag_progress = None;
                self.toast("客户端失败");
                return;
            }
        };
        tokio::spawn(async move {
            let old_id = song.id.clone();
            let (hint_artist, hint_title) = musyc_library::filename_hint(&origin);
            let title = hint_title
                .clone()
                .filter(|s| !s.trim().is_empty())
                .map(|t| musyc_library::clean_match_title(&t))
                .unwrap_or_else(|| musyc_library::clean_match_title(&song.name));
            let artist = hint_artist
                .clone()
                .or_else(|| song.artists.first().cloned())
                .unwrap_or_default();
            let query = if artist.trim().is_empty() {
                title.clone()
            } else {
                format!("{title} {}", artist.trim())
            };

            let hint_title_clean = hint_title.as_deref().map(musyc_library::clean_match_title);
            let updated = match resolve_and_convert(
                ConvertClients {
                    api: &api,
                    http: &http,
                    web: &web,
                },
                &song,
                hint_artist.as_deref(),
                hint_title_clean.as_deref(),
                &query,
                ConvertCtx {
                    source_dir: &source_dir,
                    output_dir: &output_dir,
                },
            )
            .await
            {
                Ok(update) => Some(update),
                Err(e) => {
                    fallback::log_failure(&log_dir, &origin, &query, &e);
                    None
                }
            };

            let message = match &updated {
                Some(_update) => "副本生成".to_string(),
                None => "无匹配".to_string(),
            };
            let _ = tx.send(Event::TagResult {
                seq,
                old_id,
                updated,
                message,
            });
        });
    }
    pub(super) fn tag_all_missing(&mut self) {
        if self.view != View::Local || self.search.is_some() {
            self.toast("本地补全标签");
            return;
        }
        if self.tagging {
            self.toast("正在补全中");
            return;
        }
        if self.local_loading {
            self.toast("扫描中…");
            return;
        }
        let pending: Vec<Song> = self
            .local_songs
            .iter()
            .filter(|song| self.local_needs_tag.contains(&song.id))
            .cloned()
            .collect();
        if pending.is_empty() {
            self.toast("标签已完整");
            return;
        }
        let source_dir = self.cfg.local.resolved_dir();
        let output_dir = self.cfg.local.resolved_output_dir();
        if same_dir(&source_dir, &output_dir) {
            self.toast("目录不能相同");
            return;
        }

        self.tag_seq += 1;
        let seq = self.tag_seq;
        let total = pending.len() as u64;
        self.tagging = true;
        self.toast = None;
        self.tag_progress = Some((0, total, String::new()));
        let api = self.api.clone();
        let http = self.http.clone();
        let tx = self.tx.clone();
        let log_dir = self.paths.log_dir.clone();
        let web = match fallback::external_client() {
            Ok(client) => client,
            Err(_) => {
                self.tagging = false;
                self.tag_progress = None;
                self.toast("客户端失败");
                return;
            }
        };
        let delay = self.cfg.local.tag_delay_ms.min(2000);
        tokio::spawn(async move {
            let mut fixed = 0u64;
            let mut failed = 0u64;
            let mut consecutive = 0u32;
            for (index, song) in pending.iter().enumerate() {
                let origin = local_origin(song);
                let (hint_artist, hint_title) = musyc_library::filename_hint(&origin);
                let title = hint_title
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .map(|t| musyc_library::clean_match_title(&t))
                    .unwrap_or_else(|| musyc_library::clean_match_title(&song.name));
                let artist = hint_artist
                    .clone()
                    .or_else(|| song.artists.first().cloned())
                    .unwrap_or_default();
                let query = if artist.trim().is_empty() {
                    title.clone()
                } else {
                    format!("{title} {}", artist.trim())
                };

                let mut wrote = false;
                let hint_title_clean = hint_title.as_deref().map(musyc_library::clean_match_title);
                match resolve_and_convert(
                    ConvertClients {
                        api: &api,
                        http: &http,
                        web: &web,
                    },
                    song,
                    hint_artist.as_deref(),
                    hint_title_clean.as_deref(),
                    &query,
                    ConvertCtx {
                        source_dir: &source_dir,
                        output_dir: &output_dir,
                    },
                )
                .await
                {
                    Ok(_) => {
                        consecutive = 0;
                        fixed += 1;
                        wrote = true;
                    }
                    Err(e) => {
                        consecutive += 1;
                        fallback::log_failure(&log_dir, &origin, &query, &e);
                    }
                }
                if !wrote {
                    failed += 1;
                }
                let _ = tx.send(Event::TagProgress {
                    seq,
                    done: (index + 1) as u64,
                    total,
                    name: song.name.clone(),
                });
                if consecutive >= 6 {
                    failed += pending.len().saturating_sub(index + 1) as u64;
                    break;
                }
                if delay > 0 {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
            let _ = tx.send(Event::TagDone { seq, fixed, failed });
        });
    }
}

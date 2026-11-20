use super::*;

impl App {
    pub(in crate::app) fn play_at(&mut self, index: usize, start_ms: u64, autoplay: bool) {
        let Some(song) = self.queue.get(index).cloned() else {
            return;
        };
        self.cursor = Some(index);
        self.current = Some(song.clone());
        self.loaded = false;
        self.playing = false;
        self.loading_play = true;
        // 立刻停掉上一首，避免解析新音源时旧曲还在响、旧进度也继续上报。
        self.engine.send(Cmd::Stop);
        self.pos_ms = start_ms;
        self.dur_ms = song.duration_ms;
        self.next_start_ms = start_ms;
        self.pending_autoplay = autoplay;
        self.cache_progress = None;
        self.play_seq += 1;
        let seq = self.play_seq;

        self.load_lyrics(&song);
        self.load_cover(&song);
        if song.source == SourceKind::Local {
            self.start_local(&song, start_ms, autoplay);
        } else {
            self.spawn_resolve(seq, song);
        }
        self.reveal_current();
        self.sync_mpris(true);
    }

    pub(in crate::app) fn start_local(&mut self, song: &Song, start_ms: u64, autoplay: bool) {
        let path = PathBuf::from(&song.id);
        if !path.exists() {
            // 转换副本被删或原目录移动时，直接跳过，避免卡在坏文件上。
            self.skip_unplayable("本地文件不存在，可能已被移动");
            return;
        }
        let duration = if song.duration_ms > 0 {
            song.duration_ms
        } else {
            musyc_library::read_meta(&path)
                .map(|m| m.duration_ms)
                .unwrap_or(0)
        };
        self.dur_ms = duration;
        self.engine.send(Cmd::Load {
            path,
            duration_ms: duration,
            start_ms,
            autoplay,
        });
        self.loading_play = false;
        self.loaded = true;
        self.playing = autoplay;
        if autoplay {
            self.record_history(song);
        }
        self.sync_mpris(true);
    }

    pub(in crate::app) fn spawn_resolve(&mut self, seq: u64, song: Song) {
        let resolver = self.resolver.clone();
        let tx = self.tx.clone();
        let http = self.http.clone();
        let cache = self.cache.clone();
        let quality = self.quality;
        tokio::spawn(async move {
            let source = match resolver.resolve(&song, quality).await {
                Ok(source) => source,
                Err(e) => {
                    let _ = tx.send(Event::PlayError {
                        seq,
                        message: e.to_string(),
                    });
                    return;
                }
            };
            let mut song = song;
            if let Some(duration) = source.duration_ms.filter(|d| *d > 0) {
                song.duration_ms = duration;
            }
            if let Some(album) = source.album.clone().filter(|a| !a.trim().is_empty()) {
                song.album = album;
            }
            let _ = tx.send(Event::Resolved {
                seq,
                song: song.clone(),
                url: source.url.clone(),
                provider: source.provider.clone(),
                format: source.format.clone(),
                br: source.br,
                preview: source.preview,
                duration_ms: source.duration_ms,
            });

            let key = format!("{}:{}:{}", song.key(), quality.level(), source.br);
            let tx_progress = tx.clone();
            let mut last_pct = i32::MIN;
            let result = cache
                .fetch(
                    &http,
                    &source.url,
                    &key,
                    &source.format,
                    move |done, total| {
                        let pct = total
                            .filter(|t| *t > 0)
                            .map(|t| (done.saturating_mul(100) / t) as i32)
                            .unwrap_or(-1);
                        if pct != last_pct {
                            last_pct = pct;
                            let _ = tx_progress.send(Event::CacheProgress { seq, done, total });
                        }
                    },
                )
                .await;
            match result {
                Ok(path) => {
                    let _ = tx.send(Event::CacheReady {
                        seq,
                        song,
                        path,
                        provider: source.provider,
                    });
                }
                Err(e) => {
                    let _ = tx.send(Event::PlayError {
                        seq,
                        message: e.to_string(),
                    });
                }
            }
        });
    }

    /// 播放失败时自动跳到下一首；连续失败达到队列长度或 8 首后停止。
    pub(in crate::app) fn skip_unplayable(&mut self, _message: impl Into<String>) {
        self.failed_streak += 1;
        self.loading_play = false;
        self.loaded = false;
        self.playing = false;
        self.cache_progress = None;
        let limit = self.queue.len().clamp(1, 8) as u32;
        if self.queue.is_empty() || self.failed_streak >= limit {
            self.failed_streak = 0;
            self.toast("播放失败");
            self.sync_mpris(true);
            return;
        }
        self.toast("跳过失败");
        self.next_song(true);
    }

    pub(in crate::app) fn toggle_play(&mut self) {
        if self.loading_play {
            self.toast("正在加载，请稍候…");
            return;
        }
        match self.current.clone() {
            None => {
                if self.queue.is_empty() {
                    self.toast("先搜索或打开一首歌");
                } else {
                    self.play_at(0, 0, true);
                }
            }
            Some(_) if !self.loaded => {
                let index = self.cursor.unwrap_or(0);
                let start = self.pos_ms;
                self.play_at(index, start, true);
            }
            Some(_) => {
                self.playing = !self.playing;
                self.engine.send(Cmd::Toggle);
                self.sync_mpris(true);
            }
        }
    }

    pub(in crate::app) fn seek_to(&mut self, ms: u64) {
        let target = if self.dur_ms > 0 {
            ms.min(self.dur_ms)
        } else {
            ms
        };
        self.pos_ms = target;
        if self.loaded {
            self.engine.send(Cmd::Seek(Duration::from_millis(target)));
            if let Some(tx) = &self.mpris_cmd {
                let _ = tx.send(MprisCmd::Seeked(target));
            }
        }
    }

    pub(in crate::app) fn seek_by(&mut self, delta_ms: i64) {
        let pos = (self.pos_ms as i64 + delta_ms).max(0) as u64;
        self.seek_to(pos);
    }

    pub(in crate::app) fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.5);
        self.cfg.player.volume = self.volume;
        self.engine.send(Cmd::Volume(self.volume));
        self.toast("音量");
        self.sync_mpris(true);
    }
}

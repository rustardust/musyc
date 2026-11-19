use super::*;

impl App {
    pub(super) fn play_list(&mut self, songs: Vec<Song>, index: usize) {
        if songs.is_empty() || index >= songs.len() {
            return;
        }
        self.queue = songs;
        self.play_at(index, 0, true);
    }

    pub(super) fn activate_selected(&mut self) {
        let Some(index) = self.selected_index() else {
            return;
        };
        if self.search.is_some() {
            let songs = self
                .search
                .as_ref()
                .map(|s| s.songs.clone())
                .unwrap_or_default();
            self.play_list(songs, index);
            return;
        }
        match self.view {
            View::Home => self.open_home_entry(index),
            View::Source(slot) => {
                let songs = self.slot_tracks.get(slot).cloned().unwrap_or_default();
                if !songs.is_empty() {
                    self.play_list(songs, index);
                }
            }
            View::Netease => match self.netease_page {
                NeteasePage::Browse => self.play_list(self.netease_chart.clone(), index),
                NeteasePage::Search => {
                    if let Some(playlist) = self.netease_playlist_hits.get(index).cloned() {
                        self.open_netease_playlist(playlist.id, playlist.name);
                    }
                }
                NeteasePage::Tracks => {
                    if let Some(tracks) = self.netease_tracks.clone() {
                        self.play_list(tracks.songs, index);
                    }
                }
            },
            View::Indie => match self.indie_page {
                IndiePage::Playlists => self.open_indie_playlist(index),
                IndiePage::Tracks => {
                    if let Some(tracks) = self.indie_tracks.clone() {
                        self.play_list(tracks.songs, index);
                    }
                }
            },
            View::StreetVoiceRecommend => self.play_list(self.sv_recommend.clone(), index),
            View::Local => self.play_list(self.local_songs.clone(), index),
            View::Recent => self.play_list(self.history.clone(), index),
        }
    }

    // ---------- 播放 ----------

    pub(super) fn play_at(&mut self, index: usize, start_ms: u64, autoplay: bool) {
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

    pub(super) fn start_local(&mut self, song: &Song, start_ms: u64, autoplay: bool) {
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

    pub(super) fn spawn_resolve(&mut self, seq: u64, song: Song) {
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
    /// 播放失败时自动跳到下一首；连续失败达到队列长度或 8 首后停止。
    pub(super) fn skip_unplayable(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.failed_streak += 1;
        self.loading_play = false;
        self.loaded = false;
        self.playing = false;
        self.cache_progress = None;
        let limit = self.queue.len().clamp(1, 8) as u32;
        if self.queue.is_empty() || self.failed_streak >= limit {
            self.failed_streak = 0;
            self.toast(format!("连续播放失败，已停止：{message}"));
            self.sync_mpris(true);
            return;
        }
        self.toast(format!("跳过无法播放的歌曲：{message}"));
        self.next_song(true);
    }

    /// 解析出时长 / 专辑后，把队列和各个列表里的同一首歌一起更新。
    /// 解析出时长 / 专辑后，把队列和各个列表里的同一首歌一起更新。
    pub(super) fn patch_song(&mut self, key: &str, duration: Option<u64>, album: Option<&str>) {
        fn apply(song: &mut Song, key: &str, duration: Option<u64>, album: Option<&str>) {
            if song.key() != key {
                return;
            }
            if let Some(duration) = duration {
                song.duration_ms = duration;
            }
            if let Some(album) = album.filter(|a| !a.trim().is_empty()) {
                song.album = album.to_string();
            }
        }
        let patch = |song: &mut Song| apply(song, key, duration, album);
        if let Some(song) = &mut self.current {
            patch(song);
        }
        for song in &mut self.queue {
            patch(song);
        }
        for song in &mut self.netease_chart {
            patch(song);
        }
        for song in &mut self.sv_recommend {
            patch(song);
        }
        if let Some(opened) = &mut self.netease_tracks {
            for song in &mut opened.songs {
                patch(song);
            }
        }
        if let Some(opened) = &mut self.indie_tracks {
            for song in &mut opened.songs {
                patch(song);
            }
        }
        for songs in &mut self.slot_tracks {
            for song in songs.iter_mut() {
                patch(song);
            }
        }
        if let Some(search) = &mut self.search {
            for song in &mut search.songs {
                patch(song);
            }
        }
        for song in &mut self.history {
            patch(song);
        }
        for song in &mut self.local_songs {
            patch(song);
        }
    }

    pub(super) fn next_song(&mut self, auto: bool) {
        if self.queue.is_empty() {
            return;
        }
        let index = self.cursor.unwrap_or(0).min(self.queue.len() - 1);
        let next = match self.mode {
            PlayMode::Single if auto => Some(index),
            PlayMode::Single => Some((index + 1) % self.queue.len()),
            PlayMode::ListLoop => Some((index + 1) % self.queue.len()),
            PlayMode::Sequential => {
                if index + 1 < self.queue.len() {
                    Some(index + 1)
                } else {
                    None
                }
            }
            PlayMode::Shuffle => {
                if self.queue.len() == 1 {
                    Some(0)
                } else {
                    let mut next = index;
                    while next == index {
                        next = rand::random_range(0..self.queue.len());
                    }
                    Some(next)
                }
            }
        };
        match next {
            Some(index) => self.play_at(index, 0, true),
            None => {
                self.playing = false;
                self.loaded = false;
                self.toast("队列播放完毕");
                self.sync_mpris(true);
            }
        }
    }

    pub(super) fn previous_song(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let index = self.cursor.unwrap_or(0).min(self.queue.len() - 1);
        let previous = if index == 0 {
            self.queue.len() - 1
        } else {
            index - 1
        };
        self.play_at(previous, 0, true);
    }

    pub(super) fn toggle_play(&mut self) {
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

    pub(super) fn seek_to(&mut self, ms: u64) {
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

    pub(super) fn seek_by(&mut self, delta_ms: i64) {
        let pos = (self.pos_ms as i64 + delta_ms).max(0) as u64;
        self.seek_to(pos);
    }

    pub(super) fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.5);
        self.cfg.player.volume = self.volume;
        self.engine.send(Cmd::Volume(self.volume));
        self.toast(format!("音量 {:.0}%", self.volume * 100.0));
        self.sync_mpris(true);
    }

    pub(super) fn cycle_mode(&mut self) {
        self.set_mode(self.mode.cycle());
    }

    pub(super) fn set_mode(&mut self, mode: PlayMode) {
        self.mode = mode;
        self.cfg.player.mode = mode;
        self.toast(format!("播放模式：{}", mode.label()));
        self.sync_mpris(true);
    }

    pub(super) fn set_shuffle(&mut self, shuffle: bool) {
        let mode = if shuffle {
            PlayMode::Shuffle
        } else if self.mode == PlayMode::Shuffle {
            PlayMode::ListLoop
        } else {
            self.mode
        };
        self.set_mode(mode);
    }

    pub(super) fn record_history(&mut self, song: &Song) {
        if let Err(e) = history::append(&self.paths.history_file, song) {
            tracing::warn!("写入历史失败: {e}");
        }
        self.history = history::recent(&self.paths.history_file, 50);
    }

    /// 仅当列表展示的是“最近播放”时允许删除。
    /// 仅当列表展示的是“最近播放”时允许删除。
    pub(super) fn remove_history_selected(&mut self) {
        if self.view != View::Recent || self.search.is_some() {
            self.toast("只能删除最近播放中的记录");
            return;
        }
        let Some(index) = self.selected_index() else {
            return;
        };
        let Some(song) = self.history.get(index).cloned() else {
            return;
        };
        match history::remove(&self.paths.history_file, &song.key()) {
            Ok(0) => self.toast("记录已不存在"),
            Ok(_) => {
                self.history = history::recent(&self.paths.history_file, 50);
                self.ensure_selection();
                self.toast(format!("已从最近播放移除：{}", song.name));
            }
            Err(e) => self.toast(format!("删除记录失败：{e}")),
        }
    }
}

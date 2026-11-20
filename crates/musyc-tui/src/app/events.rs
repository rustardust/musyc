use super::*;

impl App {
    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.on_key(key),
            Event::Resize => {}
            Event::SearchSongs {
                seq,
                query,
                autoplay,
                result,
            } => {
                if seq != self.search_seq {
                    return;
                }
                self.search_loading = false;
                match result {
                    Ok(songs) => {
                        if autoplay && !songs.is_empty() {
                            self.search = Some(SearchResults {
                                query: query.clone(),
                                songs: songs.clone(),
                            });
                            self.queue = songs;
                            self.play_at(0, 0, true);
                            self.list_state.select(Some(0));
                        } else {
                            let empty = songs.is_empty();
                            self.search = Some(SearchResults { query, songs });
                            self.list_state.select(Some(0));
                            self.error = None;
                            if empty {
                                self.toast("没有找到歌曲");
                            }
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("搜索失败");
                    }
                }
                self.ensure_selection();
            }
            Event::OpenSongs { seq, title, result } => {
                if seq != self.open_seq {
                    return;
                }
                match result {
                    Ok(songs) if !songs.is_empty() => self.adopt_queue(title, songs),
                    Ok(_) => self.toast("无可播放"),
                    Err(_) => self.toast("打开失败"),
                }
            }
            Event::HomeSlot { seq, slot, result } => {
                if slot >= HOME_ONLINE_SLOTS {
                    return;
                }
                if seq != self.slot_seq[slot] {
                    return;
                }
                self.slot_loading[slot] = false;
                match result {
                    Ok(songs) => {
                        self.slot_tracks[slot] = songs;
                        self.slot_loaded[slot] = true;
                        self.slot_error[slot] = None;
                    }
                    Err(e) => {
                        self.slot_error[slot] = Some(e.clone());
                        self.toast("加载失败");
                    }
                }
                self.ensure_selection();
            }
            Event::NeteaseChart { seq, result } => {
                if seq != self.netease_chart_seq {
                    return;
                }
                self.netease_chart_loading = false;
                match result {
                    Ok(songs) => {
                        self.netease_chart = songs;
                        self.netease_chart_loaded = true;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("加载失败");
                    }
                }
                self.ensure_selection();
            }
            Event::NeteasePlaylistHits { seq, query, result } => {
                if seq != self.netease_seq {
                    return;
                }
                self.netease_loading = false;
                match result {
                    Ok(playlists) => {
                        let empty = playlists.is_empty();
                        self.netease_playlist_hits = playlists;
                        self.netease_query = query;
                        self.error = None;
                        if empty {
                            self.toast("没有找到歌单");
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("搜索失败");
                    }
                }
                self.ensure_selection();
            }
            Event::NeteaseTracks {
                seq,
                id,
                title,
                result,
            } => {
                if seq != self.netease_seq {
                    return;
                }
                self.netease_loading = false;
                match result {
                    Ok(songs) => {
                        let empty = songs.is_empty();
                        self.netease_tracks = Some(Opened { id, title, songs });
                        self.error = None;
                        if empty {
                            self.toast("歌单为空");
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("打开失败");
                    }
                }
                self.ensure_selection();
            }
            Event::IndieCatalog { seq, result } => {
                if seq != self.indie_seq {
                    return;
                }
                self.indie_loading = false;
                match result {
                    Ok(playlists) => {
                        self.indie_playlists = playlists;
                        self.indie_loaded = true;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("加载失败");
                    }
                }
                self.ensure_selection();
            }
            Event::StreetVoiceRecommend { seq, result } => {
                if seq != self.sv_recommend_seq {
                    return;
                }
                self.sv_recommend_loading = false;
                match result {
                    Ok(songs) => {
                        self.sv_recommend = songs;
                        self.sv_recommend_loaded = true;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("加载失败");
                    }
                }
                self.ensure_selection();
            }
            Event::LocalSongs { seq, result } => {
                if seq != self.local_seq {
                    return;
                }
                self.local_loading = false;
                match result {
                    Ok(scan) => {
                        self.local_songs = scan.songs;
                        self.local_needs_tag = scan.needs_tag.into_iter().collect();
                        self.local_loaded = true;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("扫描失败");
                    }
                }
                self.ensure_selection();
            }
            Event::TagResult {
                seq,
                old_id,
                updated,
                message,
            } => {
                if seq != self.tag_seq {
                    return;
                }
                self.tagging = false;
                self.tag_progress = None;
                if let Some(update) = updated {
                    self.local_needs_tag.remove(&old_id);
                    if let Some(song) = self.local_songs.iter_mut().find(|s| s.id == old_id) {
                        song.id = update.id.clone();
                        song.origin = update.origin.clone();
                        song.name = update.title.clone();
                        song.artists = update.artists.clone();
                        song.album = update.album.clone();
                    }
                    if let Some(current) = &mut self.current
                        && current.id == old_id
                    {
                        current.id = update.id.clone();
                        current.origin = update.origin.clone();
                        current.name = update.title;
                        current.artists = update.artists;
                        current.album = update.album;
                        let current = current.clone();
                        // 正在播放的原文件刚刚被补全写入正文，歌词/封面只存在于
                        // 新生成的转换副本里，立即重读，歌词面板即时出现歌词。
                        self.load_lyrics(&current);
                        let old_key = format!("本地:{old_id}");
                        self.cover_cache.remove(&old_key);
                        self.load_cover(&current);
                    }
                    self.toast(message);
                    // 标签写入正文后立刻重扫本地，补全结果即时显示。
                    self.scan_local();
                } else {
                    self.toast(message);
                }
            }
            Event::TagProgress {
                seq,
                done,
                total,
                name,
            } => {
                if seq == self.tag_seq {
                    self.tag_progress = Some((done, total, name));
                }
            }
            Event::TagDone { seq, .. } => {
                if seq != self.tag_seq {
                    return;
                }
                self.tagging = false;
                self.tag_progress = None;
                self.toast("补全完成");
                self.scan_local();
            }
            Event::Lyric { key, raw } => {
                let is_current = self.current.as_ref().is_some_and(|s| s.key() == key);
                if is_current && self.lyrics_key.as_deref() == Some(key.as_str()) {
                    self.lyrics = Lyrics::parse(&raw.lrc, &raw.tlyric);
                }
            }
            Event::Cover { key, result } => {
                if self.current.as_ref().is_some_and(|s| s.key() == key) {
                    match result {
                        Ok(art) => {
                            let art = Arc::new(art);
                            self.cover_cache.insert(key.clone(), art.clone());
                            self.cover = Some((key, art));
                        }
                        Err(_) => self.cover = None,
                    }
                }
            }
            Event::ApiStatus { ok, message: _ } => {
                if !ok {
                    self.toast("接口异常");
                }
            }
            Event::Resolved {
                seq,
                song,
                duration_ms,
                preview,
                ..
            } => {
                if seq != self.play_seq {
                    return;
                }
                let key = song.key();
                self.patch_song(
                    &key,
                    duration_ms.filter(|d| *d > 0),
                    Some(song.album.as_str()).filter(|a| !a.trim().is_empty()),
                );
                if preview {
                    self.toast("仅获取到试听片段");
                }
                self.cache_progress = Some((0, None));
            }
            Event::CacheProgress { seq, done, total } => {
                if seq != self.play_seq {
                    return;
                }
                self.cache_progress = Some((done, total));
            }
            Event::CacheReady {
                seq,
                song,
                path,
                provider: _,
            } => {
                if seq != self.play_seq {
                    return;
                }
                let start = self.next_start_ms;
                let autoplay = self.pending_autoplay;
                self.next_start_ms = 0;
                self.loading_play = false;
                self.loaded = true;
                self.playing = autoplay;
                self.cache_progress = None;
                self.engine.send(Cmd::Load {
                    path,
                    duration_ms: song.duration_ms.max(self.dur_ms),
                    start_ms: start,
                    autoplay,
                });
                if autoplay {
                    self.record_history(&song);
                }
                self.sync_mpris(true);
            }
            Event::PlayError { seq, message } => {
                if seq != self.play_seq {
                    return;
                }
                self.skip_unplayable(message);
            }
            Event::DownloadProgress { seq, done, total } => {
                if seq != self.download_seq || self.downloading.is_none() {
                    return;
                }
                self.download_progress = Some((done, total));
            }
            Event::DownloadDone { seq, .. } => {
                if seq != self.download_seq {
                    return;
                }
                self.downloading = None;
                self.download_progress = None;
                self.toast("下载完成");
            }
            Event::DownloadFailed { seq, .. } => {
                if seq != self.download_seq {
                    return;
                }
                self.downloading = None;
                self.download_progress = None;
                self.toast("下载失败");
            }
            Event::Player(event) => self.on_player(event),
            Event::Action(action) => self.handle_action(action),
        }
    }

    pub(super) fn on_player(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::Ready => {
                // 解码成功才清零；连续坏文件必须在达到上限后停下。
                self.loading_play = false;
                self.failed_streak = 0;
            }
            PlayerEvent::Ended => {
                self.pos_ms = self.dur_ms;
                self.next_song(true);
            }
            PlayerEvent::Error(message) => {
                self.skip_unplayable(message);
            }
            PlayerEvent::Position {
                pos_ms,
                dur_ms,
                playing,
            } => {
                // 加载新曲期间，引擎可能还在上报上一首的进度，忽略。
                if self.loading_play {
                    return;
                }
                self.pos_ms = pos_ms;
                if dur_ms > 0 {
                    self.dur_ms = dur_ms;
                    // 播放器拿到了真实时长：回写到当前歌曲和所有包含它的列表，
                    // 让列表里原本显示 --:-- 的时长变成实际值。
                    if self
                        .current
                        .as_ref()
                        .is_some_and(|song| song.duration_ms == 0)
                    {
                        let key = self.current.as_ref().map(|s| s.key());
                        if let Some(key) = key {
                            self.patch_song(&key, Some(dur_ms), None);
                        }
                    }
                }
                self.playing = playing;
            }
        }
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::TogglePlay => self.toggle_play(),
            Action::Next => self.next_song(false),
            Action::Previous => self.previous_song(),
            Action::SeekDelta(delta) => self.seek_by(delta),
            Action::SeekTo(ms) => self.seek_to(ms),
            Action::SetVolume(volume) => self.set_volume(volume),
            Action::SetMode(mode) => self.set_mode(mode),
            Action::SetShuffle(shuffle) => self.set_shuffle(shuffle),
        }
    }
}

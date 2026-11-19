use super::*;

impl App {
    pub(super) fn check_api(&self) {
        let api = self.api.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = api.ping().await;
            let _ = tx.send(Event::ApiStatus {
                ok: result.is_ok(),
                message: match &result {
                    Ok(_) => format!("已连接 API：{}", api.base_url()),
                    Err(e) => format!("API 未连接：{e}"),
                },
            });
        });
    }

    // ---------- 列表视图 ----------

    pub(super) fn ensure_home_data(&mut self) {
        // 前三个槽位按绑定拉取；后两个固定视图各自加载
        for slot in 0..HOME_ONLINE_SLOTS {
            self.ensure_slot(slot);
        }
        if !self.local_loaded && !self.local_loading {
            self.scan_local();
        }
        self.history = history::recent(&self.paths.history_file, 50);
    }

    /// 拉取第 `slot` 个首页槽位的内容；未加载且未在拉取时才 spawn。
    pub(super) fn ensure_slot(&mut self, slot: usize) {
        if slot >= HOME_ONLINE_SLOTS {
            return;
        }
        if !self.slot_loaded[slot] && !self.slot_loading[slot] {
            self.load_slot(slot);
        }
    }

    pub(super) fn load_slot(&mut self, slot: usize) {
        self.slot_seq[slot] += 1;
        let seq = self.slot_seq[slot];
        self.slot_loading[slot] = true;
        self.slot_error[slot] = None;
        let bound = home_entry_id(&self.cfg.home.entries, slot);
        let api = self.api.clone();
        let http = self.http.clone();
        let tx = self.tx.clone();
        match bound.as_str() {
            musyc_core::sources::BUILTIN_NETEASE => {
                tokio::spawn(async move {
                    let result = api.original_chart(30).await.map_err(|e| e.to_string());
                    let _ = tx.send(Event::HomeSlot { seq, slot, result });
                });
            }
            musyc_core::sources::BUILTIN_STREETVOICE => {
                tokio::spawn(async move {
                    let result = api.streetvoice_recommend().await.map_err(|e| e.to_string());
                    let _ = tx.send(Event::HomeSlot { seq, slot, result });
                });
            }
            musyc_core::sources::BUILTIN_INDIE => {
                tokio::spawn(async move {
                    // 雀乐：拉一次目录，把所有期的歌曲扁平化作为「雀乐推荐」
                    let result = async {
                        let catalog = api.indie_catalog().await.map_err(|e| e.to_string())?;
                        // 雀乐最新一期：取目录第一项的歌曲，避免把十期扁平成长队
                        let songs = catalog
                            .into_iter()
                            .next()
                            .map(|p| p.songs)
                            .unwrap_or_default();
                        Ok::<Vec<Song>, String>(songs)
                    }
                    .await;
                    let _ = tx.send(Event::HomeSlot { seq, slot, result });
                });
            }
            _ => {
                let source = match self.slot_source(slot) {
                    Some(s) => s,
                    None => {
                        self.slot_loading[slot] = false;
                        self.slot_error[slot] = Some(format!("未找到源「{bound}」"));
                        return;
                    }
                };
                // netease-playlist kind：复用 playlist_tracks 快速通道
                match source.kind {
                    musyc_core::SourceDefinitionKind::NeteasePlaylist => {
                        let id = source.playlist_id.clone();
                        tokio::spawn(async move {
                            let result = api
                                .playlist_tracks(&id, 1000, 0)
                                .await
                                .map_err(|e| e.to_string());
                            let _ = tx.send(Event::HomeSlot { seq, slot, result });
                        });
                    }
                    musyc_core::SourceDefinitionKind::StreetVoiceCat => {
                        let cat = if source.cat.trim().is_empty() {
                            "recommend".to_string()
                        } else {
                            source.cat.trim().to_string()
                        };
                        tokio::spawn(async move {
                            let result = if cat == "recommend" {
                                api.streetvoice_recommend().await.map_err(|e| e.to_string())
                            } else {
                                api.streetvoice_cat(&cat).await.map_err(|e| e.to_string())
                            };
                            let _ = tx.send(Event::HomeSlot { seq, slot, result });
                        });
                    }
                    _ => {
                        tokio::spawn(async move {
                            let result = load_custom_source(&source, &http)
                                .await
                                .map_err(|e| e.to_string());
                            let _ = tx.send(Event::HomeSlot { seq, slot, result });
                        });
                    }
                }
            }
        }
    }

    pub(super) fn ensure_view_data(&mut self) {
        match self.view {
            View::Home => self.ensure_home_data(),
            View::Source(slot) => {
                if !self.slot_loaded[slot] && !self.slot_loading[slot] {
                    self.load_slot(slot);
                }
            }
            View::Netease if self.netease_page == NeteasePage::Browse => {
                if !self.netease_chart_loaded && !self.netease_chart_loading {
                    self.load_netease_chart();
                }
            }
            View::Indie if self.indie_page == IndiePage::Playlists => {
                if !self.indie_loaded && !self.indie_loading {
                    self.load_indie();
                }
            }
            View::StreetVoiceRecommend
                if !self.sv_recommend_loaded && !self.sv_recommend_loading =>
            {
                self.load_streetvoice();
            }
            View::Local if !self.local_loaded && !self.local_loading => self.scan_local(),
            View::Recent => {
                self.history = history::recent(&self.paths.history_file, 50);
            }
            _ => {}
        }
    }

    pub(super) fn open_home_entry(&mut self, index: usize) {
        if index >= 5 {
            return;
        }
        self.remember_selection();
        self.search_seq += 1;
        self.search = None;
        self.search_loading = false;
        self.focus = Focus::List;
        self.input.clear();
        self.input_cursor = 0;
        self.error = None;
        if index < HOME_ONLINE_SLOTS {
            let bound = home_entry_id(&self.cfg.home.entries, index);
            // 雀乐槽位（第三个在线入口）走两级：歌单列表 → 歌曲列表；
            // 其它在线入口统一走 View::Source(slot) 扁平渲染。
            if index == 2 && bound == musyc_core::sources::BUILTIN_INDIE {
                self.view = View::Indie;
            } else {
                self.view = View::Source(index);
            }
        } else {
            self.view = if index == 3 {
                View::Local
            } else {
                View::Recent
            };
        }
        self.restore_selection();
        self.ensure_view_data();
    }

    pub(super) fn load_netease_chart(&mut self) {
        self.netease_chart_seq += 1;
        let seq = self.netease_chart_seq;
        self.netease_chart_loading = true;
        self.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = api.original_chart(30).await.map_err(|e| e.to_string());
            let _ = tx.send(Event::NeteaseChart { seq, result });
        });
    }

    pub(super) fn search_netease_playlists(&mut self, query: &str) {
        let query = query.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.remember_selection();
        self.search_seq += 1;
        self.search = None;
        self.search_loading = false;
        self.netease_seq += 1;
        let seq = self.netease_seq;
        self.netease_loading = true;
        self.view = View::Netease;
        self.netease_page = NeteasePage::Search;
        self.netease_back_to_search = false;
        if self.netease_query != query {
            self.selection_memory.insert(ListKey::NeteaseSearch, 0);
        }
        self.netease_query = query.clone();
        self.netease_playlist_hits.clear();
        self.restore_selection();
        self.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = api
                .search_playlists(&query, 50)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::NeteasePlaylistHits { seq, query, result });
        });
    }

    pub(super) fn open_netease_playlist(&mut self, id: String, title: String) {
        self.remember_selection();
        self.netease_seq += 1;
        let seq = self.netease_seq;
        self.netease_back_to_search = self.view == View::Netease
            && matches!(self.netease_page, NeteasePage::Browse | NeteasePage::Search)
            && self.netease_page == NeteasePage::Search;
        self.view = View::Netease;
        self.netease_loading = true;
        self.netease_page = NeteasePage::Tracks;
        self.netease_tracks = Some(Opened {
            id: id.clone(),
            title: title.clone(),
            songs: Vec::new(),
        });
        self.restore_selection();
        self.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = api
                .playlist_tracks(&id, 1000, 0)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::NeteaseTracks {
                seq,
                id,
                title,
                result,
            });
        });
    }

    pub(super) fn load_indie(&mut self) {
        self.indie_seq += 1;
        let seq = self.indie_seq;
        self.indie_loading = true;
        self.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = api.indie_catalog().await.map_err(|e| e.to_string());
            let _ = tx.send(Event::IndieCatalog { seq, result });
        });
    }

    pub(super) fn open_indie_playlist(&mut self, index: usize) {
        let Some(playlist) = self.indie_playlists.get(index).cloned() else {
            return;
        };
        self.remember_selection();
        self.indie_tracks = Some(Opened {
            id: playlist.id.clone(),
            title: playlist.name.clone(),
            songs: playlist.songs.clone(),
        });
        self.indie_page = IndiePage::Tracks;
        self.restore_selection();
    }

    pub(super) fn load_streetvoice(&mut self) {
        self.sv_recommend_seq += 1;
        let seq = self.sv_recommend_seq;
        self.sv_recommend_loading = true;
        self.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = api.streetvoice_recommend().await.map_err(|e| e.to_string());
            let _ = tx.send(Event::StreetVoiceRecommend { seq, result });
        });
    }

    pub(super) fn scan_local(&mut self) {
        self.local_seq += 1;
        let seq = self.local_seq;
        self.local_loading = true;
        self.error = None;
        let dir = self.cfg.local.resolved_dir();
        let output_dir = self.cfg.local.resolved_output_dir();
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            let result = scan_local_dir(&dir, &output_dir);
            let _ = tx.send(Event::LocalSongs { seq, result });
        });
    }

    pub(super) fn refresh_current(&mut self) {
        self.error = None;
        match self.view {
            View::Home => {
                for slot in 0..HOME_ONLINE_SLOTS {
                    self.slot_loaded[slot] = false;
                    self.load_slot(slot);
                }
                self.scan_local();
                self.history = history::recent(&self.paths.history_file, 50);
                self.toast("正在刷新首页…");
            }
            View::Netease => match self.netease_page {
                NeteasePage::Tracks => {
                    if let Some(tracks) = self.netease_tracks.clone() {
                        self.open_netease_playlist(tracks.id, tracks.title);
                    } else {
                        self.netease_page = NeteasePage::Browse;
                        self.load_netease_chart();
                    }
                }
                NeteasePage::Search => {
                    let query = self.netease_query.clone();
                    self.search_netease_playlists(&query);
                }
                NeteasePage::Browse => self.load_netease_chart(),
            },
            View::Indie => {
                self.indie_page = IndiePage::Playlists;
                self.indie_tracks = None;
                self.load_indie();
            }
            View::StreetVoiceRecommend => self.load_streetvoice(),
            View::Source(slot) => self.load_slot(slot),
            View::Local => self.scan_local(),
            View::Recent => {
                self.history = history::recent(&self.paths.history_file, 50);
                self.toast("已刷新最近播放");
            }
        }
    }

    // ---------- 输入与搜索 ----------

    pub(super) fn submit_input(&mut self) {
        let raw = self.input.trim().to_string();
        if raw.is_empty() {
            self.focus = Focus::List;
            return;
        }
        let mode = self.input_mode;
        self.focus = Focus::List;
        match mode {
            InputMode::Search => self.search_songs_now(&raw),
            InputMode::PlaylistSearch => self.search_netease_playlists(&raw),
            InputMode::Open => self.submit_raw(&raw),
        }
    }

    pub(super) fn submit_raw(&mut self, raw: &str) {
        let text = raw.trim();
        if text.is_empty() {
            return;
        }
        match input::classify(text) {
            Classified::M3u(path) => self.open_m3u(path),
            Classified::Local(path) => self.open_local(path),
            Classified::Playlist(id) => {
                self.open_netease_playlist(id.clone(), format!("歌单 {id}"));
            }
            Classified::Song(id) => self.open_song_id(id),
            Classified::Query(query) => self.search_songs_now(&query),
        }
        self.focus = Focus::List;
    }

    pub(super) fn search_songs_now(&mut self, query: &str) {
        let query = query.trim().to_string();
        if query.is_empty() {
            return;
        }
        let same_query = self.search.as_ref().is_some_and(|s| s.query == query);
        self.remember_selection();
        if !same_query {
            self.selection_memory.insert(ListKey::Search, 0);
        }
        self.search_seq += 1;
        let seq = self.search_seq;
        self.search_loading = true;
        self.error = None;
        self.search = Some(SearchResults {
            query: query.clone(),
            songs: Vec::new(),
        });
        self.restore_selection();
        let api = self.api.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = api
                .search_songs(&query, 50, 0)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::SearchSongs {
                seq,
                query,
                autoplay: false,
                result,
            });
        });
    }

    pub(super) fn open_song_id(&mut self, id: String) {
        self.open_seq += 1;
        let seq = self.open_seq;
        self.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = match api.song_detail(std::slice::from_ref(&id)).await {
                Ok(songs) if !songs.is_empty() => Ok(songs),
                Ok(_) => api
                    .playlist_tracks(&id, 1000, 0)
                    .await
                    .map_err(|e| e.to_string()),
                Err(e) => api
                    .playlist_tracks(&id, 1000, 0)
                    .await
                    .map_err(|_| e.to_string()),
            };
            let _ = tx.send(Event::OpenSongs {
                seq,
                title: "链接打开".into(),
                result,
            });
        });
    }

    pub(super) fn open_local(&mut self, path: PathBuf) {
        if !path.exists() {
            self.toast(format!("文件不存在：{}", path.display()));
            return;
        }
        let song = local_song(&path, None, 0);
        self.adopt_queue(format!("本地 · {}", song.name), vec![song]);
    }

    pub(super) fn open_m3u(&mut self, path: PathBuf) {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                self.toast(format!("读取歌单失败：{e}"));
                return;
            }
        };
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let entries = m3u::parse(&text, base);
        if entries.is_empty() {
            self.toast("歌单为空，或没有可识别的条目");
            return;
        }

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("M3U")
            .to_string();
        let mut songs = Vec::with_capacity(entries.len());
        let mut online: Vec<(usize, String)> = Vec::new();
        for entry in entries {
            match entry {
                m3u::M3uEntry::Local {
                    path,
                    title,
                    duration_ms,
                } => songs.push(local_song(&path, title, duration_ms)),
                m3u::M3uEntry::Online { id, title } => {
                    let index = songs.len();
                    songs.push(Song {
                        name: title.unwrap_or_else(|| format!("网易云 {id}")),
                        id: id.clone(),
                        source: SourceKind::Netease,
                        ..Song::default()
                    });
                    online.push((index, id));
                }
            }
        }

        if online.is_empty() {
            self.adopt_queue(title, songs);
            return;
        }

        self.open_seq += 1;
        let seq = self.open_seq;
        self.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        let ids: Vec<String> = online.iter().map(|(_, id)| id.clone()).collect();
        tokio::spawn(async move {
            let result = api
                .song_detail(&ids)
                .await
                .map(|details| {
                    let mut map: HashMap<String, Song> = details
                        .into_iter()
                        .map(|song| (song.id.clone(), song))
                        .collect();
                    for (index, id) in online {
                        if let Some(song) = map.remove(&id) {
                            songs[index] = song;
                        }
                    }
                    songs
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::OpenSongs { seq, title, result });
        });
    }

    pub(super) fn adopt_queue(&mut self, title: String, songs: Vec<Song>) {
        if songs.is_empty() {
            self.toast("列表为空");
            return;
        }
        let count = songs.len();
        self.queue = songs;
        self.remember_selection();
        self.search_seq += 1;
        self.search = None;
        self.search_loading = false;
        self.focus = Focus::List;
        self.input.clear();
        self.input_cursor = 0;
        self.input_mode = InputMode::Search;
        self.error = None;
        self.restore_selection();
        self.toast(format!("已打开：{title}（{count} 首）"));
        self.play_at(0, 0, true);
    }

    /// 按来源抓歌词，供下载写标签等后台任务用（不触碰界面状态）。
    pub(super) async fn lyric_for_source(
        api: &ApiClient,
        http: &reqwest::Client,
        song: &Song,
    ) -> Option<String> {
        match song.source {
            SourceKind::Local => musyc_library::read_lyrics(Path::new(&song.id)),
            SourceKind::Indie | SourceKind::Custom => {
                let url = song.lyric_url.clone()?;
                let resp = http.get(url).send().await.ok()?;
                let text = resp.text().await.ok()?;
                let trimmed = text.trim();
                (!trimmed.is_empty()).then_some(text)
            }
            SourceKind::StreetVoice => {
                let detail = api.streetvoice_detail(&song.id).await.ok()?;
                let text = detail.lyrics.trim().to_string();
                (!text.is_empty()).then_some(detail.lyrics)
            }
            _ => {
                let raw = api.lyric(&song.id).await.ok()?;
                let text = raw.lrc.trim().to_string();
                (!text.is_empty()).then_some(raw.lrc)
            }
        }
    }

    pub(super) fn load_lyrics(&mut self, song: &Song) {
        let key = song.key();
        self.lyrics = Lyrics::default();
        self.lyrics_key = Some(key.clone());
        self.lyrics_scroll = 0;
        match song.source {
            SourceKind::Local => {
                if let Some(text) = musyc_library::read_lyrics(Path::new(&song.id)) {
                    self.lyrics = Lyrics::parse(&text, "");
                }
            }
            SourceKind::Indie | SourceKind::Custom => {
                let Some(url) = song.lyric_url.clone() else {
                    return;
                };
                let http = self.http.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    if let Ok(resp) = http.get(url).send().await
                        && let Ok(text) = resp.text().await
                    {
                        let _ = tx.send(Event::Lyric {
                            key,
                            raw: RawLyric {
                                lrc: text,
                                tlyric: String::new(),
                            },
                        });
                    }
                });
            }
            SourceKind::StreetVoice => {
                let api = self.api.clone();
                let tx = self.tx.clone();
                let id = song.id.clone();
                tokio::spawn(async move {
                    if let Ok(detail) = api.streetvoice_detail(&id).await
                        && !detail.lyrics.trim().is_empty()
                    {
                        let _ = tx.send(Event::Lyric {
                            key,
                            raw: RawLyric {
                                lrc: detail.lyrics,
                                tlyric: String::new(),
                            },
                        });
                    }
                });
            }
            _ => {
                let api = self.api.clone();
                let tx = self.tx.clone();
                let id = song.id.clone();
                tokio::spawn(async move {
                    if let Ok(raw) = api.lyric(&id).await {
                        let _ = tx.send(Event::Lyric { key, raw });
                    }
                });
            }
        }
    }

    // ---------- 封面 ----------

    pub(super) fn load_cover(&mut self, song: &Song) {
        let key = song.key();
        if self.cover.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        if let Some(art) = self.cover_cache.get(&key) {
            self.cover = Some((key, art.clone()));
            return;
        }
        self.cover = None;
        self.cover_seq += 1;
        let tx = self.tx.clone();
        let http = self.http.clone();
        let cover_url = song.cover_url.clone();
        let local_path = (song.source == SourceKind::Local).then(|| PathBuf::from(&song.id));
        tokio::spawn(async move {
            let bytes = if let Some(path) = local_path {
                tokio::task::spawn_blocking(move || musyc_library::read_cover(&path))
                    .await
                    .ok()
                    .flatten()
            } else if let Some(url) = cover_url.filter(|u| !u.trim().is_empty()) {
                match http.get(&url).send().await {
                    Ok(resp) => resp.bytes().await.ok().map(|b| b.to_vec()),
                    Err(_) => None,
                }
            } else {
                None
            };
            let Some(bytes) = bytes else {
                let _ = tx.send(Event::Cover {
                    key,
                    result: Err("没有封面".into()),
                });
                return;
            };
            let result = match tokio::task::spawn_blocking(move || decode_cover(&bytes)).await {
                Ok(Some(art)) => Ok(art),
                Ok(None) => Err("封面解码失败".into()),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(Event::Cover { key, result });
        });
    }

    // ---------- 本地标签补全 ----------

    pub(super) fn tag_selected(&mut self) {
        if self.view != View::Local || self.search.is_some() {
            self.toast("只能在本地列表里补全标签");
            return;
        }
        if self.tagging {
            self.toast("已有补全任务在进行");
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
            self.toast("本地文件不存在");
            return;
        }
        let source_dir = self.cfg.local.resolved_dir();
        let output_dir = self.cfg.local.resolved_output_dir();
        if same_dir(&source_dir, &output_dir) {
            self.toast("转换目录不能与源目录相同，请修改配置 [local] output_dir");
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
            Err(e) => {
                self.tagging = false;
                self.tag_progress = None;
                self.toast(&e);
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
                Some(update) => format!(
                    "已生成转换副本：{} — {}",
                    update.title,
                    update.artists.join(" / ")
                ),
                None => format!("没有找到匹配（{query}）"),
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
            self.toast("只能在本地列表里补全标签");
            return;
        }
        if self.tagging {
            self.toast("已有补全任务在进行");
            return;
        }
        if self.local_loading {
            self.toast("正在扫描本地目录，请稍候…");
            return;
        }
        let pending: Vec<Song> = self
            .local_songs
            .iter()
            .filter(|song| self.local_needs_tag.contains(&song.id))
            .cloned()
            .collect();
        if pending.is_empty() {
            self.toast("本地标签都已完整");
            return;
        }
        let source_dir = self.cfg.local.resolved_dir();
        let output_dir = self.cfg.local.resolved_output_dir();
        if same_dir(&source_dir, &output_dir) {
            self.toast("转换目录不能与源目录相同，请修改配置 [local] output_dir");
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
            Err(e) => {
                self.tagging = false;
                self.tag_progress = None;
                self.toast(&e);
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

    // ---------- 下载 ----------

    pub(super) fn download_selected(&mut self) {
        // 优先使用光标选中的歌曲；选中行不是歌曲（首页 / 歌单列表）时回退到正在播放的歌。
        let Some(song) = self.selected_song().or_else(|| self.current.clone()) else {
            self.toast("当前没有歌曲");
            return;
        };
        if !song.source.is_online() {
            self.toast("本地文件无需下载");
            return;
        }
        if self.downloading.is_some() {
            self.toast("已有下载任务进行中");
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

    // ---------- 事件 ----------
}

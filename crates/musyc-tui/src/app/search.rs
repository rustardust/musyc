use super::*;

impl App {
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
            self.toast("文件不存在");
            return;
        }
        let song = local_song(&path, None, 0);
        self.adopt_queue(format!("本地 · {}", song.name), vec![song]);
    }
    pub(super) fn open_m3u(&mut self, path: PathBuf) {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => {
                self.toast("读取歌单失败");
                return;
            }
        };
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let entries = m3u::parse(&text, base);
        if entries.is_empty() {
            self.toast("歌单为空");
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
    pub(super) fn adopt_queue(&mut self, _title: String, songs: Vec<Song>) {
        if songs.is_empty() {
            self.toast("列表为空");
            return;
        }
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
        self.toast("已打开");
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
}

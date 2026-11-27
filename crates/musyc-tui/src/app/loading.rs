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
        let source = self.slot_source(slot);
        if source.is_none()
            && !matches!(
                bound.as_str(),
                musyc_core::sources::BUILTIN_NETEASE
                    | musyc_core::sources::BUILTIN_STREETVOICE
                    | musyc_core::sources::BUILTIN_INDIE
            )
        {
            self.slot_loading[slot] = false;
            self.slot_error[slot] = Some(format!("未找到源「{bound}」"));
            return;
        }
        let api = self.api.clone();
        let http = self.http.clone();
        let tx = self.tx.clone();
        // 所有来源走同一个入口：内置与自定义只在 provider 内分派。
        tokio::spawn(async move {
            let request = match source.as_ref() {
                Some(source) => PlaylistRequest::Custom(source),
                None => PlaylistRequest::Builtin(&bound),
            };
            let result = load_playlist(request, &api, &http)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::HomeSlot { seq, slot, result });
        });
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
                self.toast("刷新中…");
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
                self.toast("已刷新");
            }
        }
    }
}

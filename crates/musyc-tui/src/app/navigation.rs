use super::*;

impl App {
    pub(super) fn visible_len(&self) -> usize {
        if let Some(search) = &self.search {
            return search.songs.len();
        }
        match self.view {
            View::Home => 5,
            View::Source(slot) => self.slot_tracks.get(slot).map(|v| v.len()).unwrap_or(0),
            View::Netease => match self.netease_page {
                NeteasePage::Browse => self.netease_chart.len(),
                NeteasePage::Search => self.netease_playlist_hits.len(),
                NeteasePage::Tracks => self
                    .netease_tracks
                    .as_ref()
                    .map(|t| t.songs.len())
                    .unwrap_or(0),
            },
            View::Indie => match self.indie_page {
                IndiePage::Playlists => self.indie_playlists.len(),
                IndiePage::Tracks => self
                    .indie_tracks
                    .as_ref()
                    .map(|t| t.songs.len())
                    .unwrap_or(0),
            },
            View::StreetVoiceRecommend => self.sv_recommend.len(),
            View::Local => self.local_songs.len(),
            View::Recent => self.history.len(),
        }
    }

    pub(super) fn ensure_selection(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            self.list_state.select(None);
            return;
        }
        let key = self.list_key();
        let index = self
            .list_state
            .selected()
            .or_else(|| self.selection_memory.get(&key).copied())
            .unwrap_or(0)
            .min(len - 1);
        self.list_state.select(Some(index));
    }

    /// 当前列表在 `selection_memory` 中的键。
    /// 当前列表在 `selection_memory` 中的键。
    pub(super) fn list_key(&self) -> ListKey {
        if self.search.is_some() {
            return ListKey::Search;
        }
        match self.view {
            View::Home => ListKey::Home,
            View::Source(slot) => ListKey::Source(slot),
            View::Netease => match self.netease_page {
                NeteasePage::Browse => ListKey::NeteaseBrowse,
                NeteasePage::Search => ListKey::NeteaseSearch,
                NeteasePage::Tracks => ListKey::NeteaseTracks(
                    self.netease_tracks
                        .as_ref()
                        .map(|t| t.id.clone())
                        .unwrap_or_default(),
                ),
            },
            View::Indie => match self.indie_page {
                IndiePage::Playlists => ListKey::IndiePlaylists,
                IndiePage::Tracks => ListKey::IndieTracks(
                    self.indie_tracks
                        .as_ref()
                        .map(|t| t.id.clone())
                        .unwrap_or_default(),
                ),
            },
            View::StreetVoiceRecommend => ListKey::StreetVoiceRecommend,
            View::Local => ListKey::Local,
            View::Recent => ListKey::Recent,
        }
    }

    /// 切换列表前保存当前指针，返回 / 重进时恢复。
    /// 切换列表前保存当前指针，返回 / 重进时恢复。
    pub(super) fn remember_selection(&mut self) {
        if let Some(index) = self.list_state.selected() {
            self.selection_memory.insert(self.list_key(), index);
        }
    }

    pub(super) fn restore_selection(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            self.list_state.select(None);
            return;
        }
        let index = self
            .selection_memory
            .get(&self.list_key())
            .copied()
            .unwrap_or(0)
            .min(len - 1);
        self.list_state.select(Some(index));
    }

    pub(super) fn move_selection(&mut self, delta: i64) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as i64;
        let next = (current + delta).clamp(0, len as i64 - 1) as usize;
        self.list_state.select(Some(next));
    }

    pub(super) fn select_boundary(&mut self, last: bool) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        self.list_state.select(Some(if last { len - 1 } else { 0 }));
    }

    /// 当前列表里是否包含正在播放的歌曲，返回它的行号。
    /// 当前列表里是否包含正在播放的歌曲，返回它的行号。
    pub(super) fn visible_song_index(&self, key: &str) -> Option<usize> {
        if let Some(search) = &self.search {
            return search.songs.iter().position(|song| song.key() == key);
        }
        let songs: &[Song] = match self.view {
            View::Home => return None,
            View::Source(slot) => self
                .slot_tracks
                .get(slot)
                .map(|v| v.as_slice())
                .unwrap_or(&[]),
            View::Netease => match self.netease_page {
                NeteasePage::Browse => &self.netease_chart,
                NeteasePage::Search => return None,
                NeteasePage::Tracks => self
                    .netease_tracks
                    .as_ref()
                    .map(|tracks| tracks.songs.as_slice())
                    .unwrap_or(&[]),
            },
            View::Indie => match self.indie_page {
                IndiePage::Playlists => return None,
                IndiePage::Tracks => self
                    .indie_tracks
                    .as_ref()
                    .map(|tracks| tracks.songs.as_slice())
                    .unwrap_or(&[]),
            },
            View::StreetVoiceRecommend => &self.sv_recommend,
            View::Local => &self.local_songs,
            View::Recent => &self.history,
        };
        songs.iter().position(|song| song.key() == key)
    }

    /// 跟随当前歌曲：自动切歌后把列表指针移到正在播放的那一行。
    /// 跟随当前歌曲：自动切歌后把列表指针移到正在播放的那一行。
    pub(super) fn reveal_current(&mut self) {
        let Some(key) = self.current.as_ref().map(|song| song.key()) else {
            return;
        };
        if let Some(index) = self.visible_song_index(&key) {
            self.list_state.select(Some(index));
            self.selection_memory.insert(self.list_key(), index);
        }
    }

    pub(super) fn scroll_lyrics(&mut self, delta: i32) {
        if !self.show_lyrics && !self.lyrics_immersive {
            return;
        }
        self.lyrics_scroll = self.lyrics_scroll.saturating_add(delta);
    }

    pub(super) fn loading(&self) -> bool {
        if self.search.is_some() {
            return self.search_loading;
        }
        match self.view {
            View::Home => false,
            View::Source(slot) => self.slot_loading.get(slot).copied().unwrap_or(false),
            View::Netease => match self.netease_page {
                NeteasePage::Browse => self.netease_chart_loading,
                _ => self.netease_loading,
            },
            View::Indie => self.indie_loading,
            View::StreetVoiceRecommend => self.sv_recommend_loading,
            View::Local => self.local_loading,
            View::Recent => false,
        }
    }

    pub(super) fn current_list_title(&self) -> String {
        if let Some(search) = &self.search {
            return format!("搜索 · {}（{}）", search.query, search.songs.len());
        }
        match self.view {
            View::Home => "首页".to_string(),
            View::Netease => match self.netease_page {
                NeteasePage::Browse => format!("网易原创（{}）", self.netease_chart.len()),
                NeteasePage::Search => format!(
                    "网易云歌单搜索 · {}（{}）",
                    self.netease_query,
                    self.netease_playlist_hits.len()
                ),
                NeteasePage::Tracks => self
                    .netease_tracks
                    .as_ref()
                    .map(|t| format!("{}（{}）", t.title, t.songs.len()))
                    .unwrap_or_else(|| "网易云歌单".into()),
            },
            View::Indie => match self.indie_page {
                IndiePage::Playlists => format!("雀乐推荐（{}）", self.indie_playlists.len()),
                IndiePage::Tracks => self
                    .indie_tracks
                    .as_ref()
                    .map(|t| format!("{}（{}）", t.title, t.songs.len()))
                    .unwrap_or_else(|| "雀乐歌单".into()),
            },
            View::StreetVoiceRecommend => format!("街声推荐（{}）", self.sv_recommend.len()),
            View::Source(slot) => {
                let id = home_entry_id(&self.cfg.home.entries, slot);
                let name = self.source_name(&id);
                let len = self.slot_tracks.get(slot).map(|s| s.len()).unwrap_or(0);
                format!("{name}（{len}）")
            }
            View::Local => format!("本地列表（{}）", self.local_songs.len()),
            View::Recent => format!("最近播放（{}）", self.history.len()),
        }
    }

    pub(super) fn empty_message(&self) -> &'static str {
        if self.search.is_some() {
            return "没有找到歌曲 · Esc 关闭搜索";
        }
        match self.view {
            View::Home => "栏目加载中…",
            View::Netease => match self.netease_page {
                NeteasePage::Browse => "榜单为空 · R 刷新",
                NeteasePage::Search => "没有找到歌单 · Esc 返回",
                NeteasePage::Tracks => "歌单为空 · Esc 返回",
            },
            View::Indie => match self.indie_page {
                IndiePage::Playlists => "暂无雀乐歌单 · R 刷新",
                IndiePage::Tracks => "歌单为空 · Esc 返回",
            },
            View::StreetVoiceRecommend => "暂无推荐 · R 刷新",
            View::Source(slot) => match self.slot_error.get(slot).and_then(|e| e.as_deref()) {
                Some(_) => "加载失败 · R 重试",
                None => "暂无内容 · R 重试",
            },
            View::Local => "本地目录为空 · 在配置 [local] dir 指定目录",
            View::Recent => "还没有播放记录",
        }
    }

    pub(super) fn selected_index(&self) -> Option<usize> {
        self.list_state
            .selected()
            .filter(|i| *i < self.visible_len())
    }

    /// 回到首页，并尽量把光标停在与刚才栏目对应的入口上。
    pub(super) fn go_home(&mut self) {
        let previous = self.view;
        self.remember_selection();
        self.search_seq += 1;
        self.search = None;
        self.search_loading = false;
        self.focus = Focus::List;
        self.input.clear();
        self.input_cursor = 0;
        self.error = None;
        self.view = View::Home;
        let index = match previous {
            View::Source(slot) => slot,
            View::Netease => 0,
            View::StreetVoiceRecommend => 1,
            View::Indie => 2,
            View::Local => 3,
            View::Recent => 4,
            _ => 0,
        };
        self.selection_memory.insert(ListKey::Home, index);
        self.restore_selection();
        self.ensure_home_data();
    }

    /// 返回当前列表光标指向的歌曲；首页、歌单列表等没有歌曲行的视图返回 None。
    pub(super) fn selected_song(&self) -> Option<Song> {
        let index = self.selected_index()?;
        if let Some(search) = &self.search {
            return search.songs.get(index).cloned();
        }
        match self.view {
            View::Home => None,
            View::Source(slot) => self.slot_tracks.get(slot)?.get(index).cloned(),
            View::Netease => match self.netease_page {
                NeteasePage::Browse => self.netease_chart.get(index).cloned(),
                // 热门歌单列表，没有歌曲
                NeteasePage::Search => None,
                NeteasePage::Tracks => self.netease_tracks.as_ref()?.songs.get(index).cloned(),
            },
            View::Indie => match self.indie_page {
                // 专辑列表，没有歌曲
                IndiePage::Playlists => None,
                IndiePage::Tracks => self.indie_tracks.as_ref()?.songs.get(index).cloned(),
            },
            View::StreetVoiceRecommend => self.sv_recommend.get(index).cloned(),
            View::Local => self.local_songs.get(index).cloned(),
            View::Recent => self.history.get(index).cloned(),
        }
    }

    /// Esc 逐级返回：搜索 → 网易 / 雀乐层级 → 首页。
    pub(super) fn go_back(&mut self) {
        if self.search.is_some() {
            self.remember_selection();
            self.search_seq += 1;
            self.search = None;
            self.search_loading = false;
            self.error = None;
            self.restore_selection();
            return;
        }
        match self.view {
            View::Netease
                if self.netease_page == NeteasePage::Tracks && self.netease_back_to_search =>
            {
                self.remember_selection();
                self.netease_seq += 1;
                self.netease_loading = false;
                self.netease_page = NeteasePage::Search;
                self.netease_back_to_search = false;
                self.error = None;
                self.restore_selection();
            }
            View::Netease if self.netease_page != NeteasePage::Browse => {
                self.remember_selection();
                self.netease_seq += 1;
                self.netease_loading = false;
                self.netease_page = NeteasePage::Browse;
                self.netease_back_to_search = false;
                self.error = None;
                self.restore_selection();
            }
            View::Indie if self.indie_page != IndiePage::Playlists => {
                self.remember_selection();
                self.indie_page = IndiePage::Playlists;
                self.error = None;
                self.restore_selection();
            }
            View::Home => {}
            _ => self.go_home(),
        }
    }

    // ---------- 换源选择器 ----------

    pub(super) fn delete_previous(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let start = prev_boundary(&self.input, self.input_cursor);
        self.input.replace_range(start..self.input_cursor, "");
        self.input_cursor = start;
    }

    pub(super) fn delete_next(&mut self) {
        if self.input_cursor >= self.input.len() {
            return;
        }
        let end = next_boundary(&self.input, self.input_cursor);
        self.input.replace_range(self.input_cursor..end, "");
    }

    // ---------- 渲染 ----------
}

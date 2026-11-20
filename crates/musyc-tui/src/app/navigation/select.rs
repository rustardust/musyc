use super::*;

impl App {
    pub(in crate::app) fn visible_len(&self) -> usize {
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

    pub(in crate::app) fn ensure_selection(&mut self) {
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

    pub(in crate::app) fn move_selection(&mut self, delta: i64) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as i64;
        let next = (current + delta).clamp(0, len as i64 - 1) as usize;
        self.list_state.select(Some(next));
    }

    pub(in crate::app) fn select_boundary(&mut self, last: bool) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        self.list_state.select(Some(if last { len - 1 } else { 0 }));
    }

    pub(in crate::app) fn selected_index(&self) -> Option<usize> {
        self.list_state
            .selected()
            .filter(|i| *i < self.visible_len())
    }

    /// 返回当前列表光标指向的歌曲；首页、歌单列表等没有歌曲行的视图返回 None。
    pub(in crate::app) fn selected_song(&self) -> Option<Song> {
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

    /// 当前列表里是否包含正在播放的歌曲，返回它的行号。
    /// 当前列表里是否包含正在播放的歌曲，返回它的行号。
    pub(in crate::app) fn visible_song_index(&self, key: &str) -> Option<usize> {
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
    pub(in crate::app) fn reveal_current(&mut self) {
        let Some(key) = self.current.as_ref().map(|song| song.key()) else {
            return;
        };
        if let Some(index) = self.visible_song_index(&key) {
            self.list_state.select(Some(index));
            self.selection_memory.insert(self.list_key(), index);
        }
    }

    pub(in crate::app) fn scroll_lyrics(&mut self, delta: i32) {
        if !self.show_lyrics && !self.lyrics_immersive {
            return;
        }
        self.lyrics_scroll = self.lyrics_scroll.saturating_add(delta);
    }
}

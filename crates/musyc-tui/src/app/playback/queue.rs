use super::*;

impl App {
    pub(in crate::app) fn play_list(&mut self, songs: Vec<Song>, index: usize) {
        if songs.is_empty() || index >= songs.len() {
            return;
        }
        self.queue = songs;
        self.play_at(index, 0, true);
    }

    pub(in crate::app) fn activate_selected(&mut self) {
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

    pub(in crate::app) fn next_song(&mut self, auto: bool) {
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

    pub(in crate::app) fn previous_song(&mut self) {
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

    pub(in crate::app) fn cycle_mode(&mut self) {
        self.set_mode(self.mode.cycle());
    }

    pub(in crate::app) fn set_mode(&mut self, mode: PlayMode) {
        self.mode = mode;
        self.cfg.player.mode = mode;
        self.toast("模式");
        self.sync_mpris(true);
    }

    pub(in crate::app) fn set_shuffle(&mut self, shuffle: bool) {
        let mode = if shuffle {
            PlayMode::Shuffle
        } else if self.mode == PlayMode::Shuffle {
            PlayMode::ListLoop
        } else {
            self.mode
        };
        self.set_mode(mode);
    }

    /// 解析出时长 / 专辑后，把队列和各个列表里的同一首歌一起更新。
    /// 解析出时长 / 专辑后，把队列和各个列表里的同一首歌一起更新。
    pub(in crate::app) fn patch_song(
        &mut self,
        key: &str,
        duration: Option<u64>,
        album: Option<&str>,
    ) {
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
}

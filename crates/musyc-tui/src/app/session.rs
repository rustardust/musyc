use super::*;

impl App {
    pub(super) fn start_mpris(&mut self) {
        if self.mpris_cmd.is_some() {
            return;
        }
        self.mpris_cmd = mpris::spawn(self.tx.clone());
        if self.mpris_cmd.is_none() {
            self.toast("MPRIS 失败");
        }
        self.mpris_last_sync = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .unwrap_or_else(Instant::now);
        self.sync_mpris(true);
    }

    pub(super) fn sync_mpris(&mut self, force: bool) {
        let Some(tx) = &self.mpris_cmd else {
            return;
        };
        if !force && self.mpris_last_sync.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.mpris_last_sync = Instant::now();
        let (title, artists, album, art_url) = match &self.current {
            Some(song) => (
                song.name.clone(),
                song.artists.clone(),
                song.album.clone(),
                song.cover_url.clone(),
            ),
            None => (String::new(), Vec::new(), String::new(), None),
        };
        let loop_status = match self.mode {
            PlayMode::Single => MprisLoop::Track,
            PlayMode::Sequential => MprisLoop::None,
            PlayMode::ListLoop | PlayMode::Shuffle => MprisLoop::Playlist,
        };
        let state = MprisState {
            title,
            artists,
            album,
            duration_ms: self.dur_ms,
            position_ms: self.pos_ms,
            art_url,
            playing: self.playing,
            stopped: self.current.is_none(),
            volume: self.volume,
            loop_status,
            shuffle: self.mode == PlayMode::Shuffle,
            can_next: !self.queue.is_empty(),
            can_prev: !self.queue.is_empty(),
        };
        let _ = tx.send(MprisCmd::Update(Box::new(state)));
    }

    pub(super) fn save_session(&self) {
        if !self.cfg.ui.restore_session {
            return;
        }
        let session = SessionFile {
            queue: self.queue.clone(),
            index: self.cursor,
            pos_ms: self.pos_ms,
        };
        match serde_json::to_string_pretty(&session) {
            Ok(json) => {
                let _ = Paths::ensure_parent(&self.paths.session_file);
                if let Err(e) = std::fs::write(&self.paths.session_file, json) {
                    tracing::warn!("保存会话失败 {}: {e}", self.paths.session_file.display());
                }
            }
            Err(e) => tracing::warn!("序列化会话失败: {e}"),
        }
    }
}
pub(super) fn load_session(path: &Path) -> Option<SessionFile> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

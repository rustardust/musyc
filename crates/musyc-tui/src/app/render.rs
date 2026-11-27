use super::*;

impl App {
    pub fn render(&mut self, frame: &mut Frame) {
        self.ensure_selection();
        let area = frame.area();
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            ui::render_too_small(frame, area, &THEME);
            return;
        }

        let search_visible = self.focus == Focus::Input;
        let search_h = u16::from(search_visible);
        let player_h = 2;
        let fixed = search_h + player_h;
        let available = area.height.saturating_sub(fixed);
        // 歌词纯享模式：歌词面板向上拓展到全部内容区；所有视图（含搜索结果）一致生效。
        let immersive = self.lyrics_immersive;
        let home = !immersive && self.view == View::Home && self.search.is_none();
        let (list_h, lyrics_h) = if immersive {
            (0, available)
        } else if home {
            // 首页歌词关闭时，主页内容在整个主体区垂直居中；
            // 歌词打开时保持紧凑 9 行（含顶部一行空行）在上方，歌词拿剩余行数。
            if self.show_lyrics {
                let list = 9.min(available);
                (list, available - list)
            } else {
                (available, 0)
            }
        } else if self.show_lyrics {
            let min_list = 5.min(available);
            let max_list = available.saturating_sub(5).max(min_list);
            let desired = ((available as u32 * 40 / 100) as u16).max(min_list);
            let list = desired.min(max_list).min(available);
            (list, available - list)
        } else {
            (available, 0)
        };
        // 歌词区用 Min 承接所有剩余行数：歌词关闭时也能保证播放条紧贴底部。
        let chunks = Layout::vertical([
            Constraint::Length(search_h),
            Constraint::Length(list_h),
            Constraint::Min(lyrics_h),
            Constraint::Length(player_h),
        ])
        .split(area);
        let search_area = chunks[0];
        let list_area = chunks[1];
        let lyrics_area = chunks[2];
        let player_area = chunks[3];

        if search_visible {
            ui::render_search(
                frame,
                search_area,
                &THEME,
                &self.input,
                self.input_cursor,
                self.input_mode,
                self.focus,
                false,
            );
        }

        let title = self.current_list_title();
        let current_key = self.current.as_ref().map(|song| song.key());
        let loading = self.loading();
        let empty = self.empty_message();
        let error = self.error.as_deref();

        if immersive {
            // 纯享模式：列表区不渲染，整个内容区交给歌词。
        } else if let Some(search) = &self.search {
            ui::render_songs(
                frame,
                list_area,
                &THEME,
                &search.songs,
                current_key.as_deref(),
                &mut self.list_state,
                &title,
                loading,
                error,
                empty,
            );
        } else {
            match self.view {
                View::Home => {
                    let labels_vec = self.home_labels();
                    let labels: Vec<&str> = labels_vec.iter().map(|s| s.as_str()).collect();
                    ui::render_home(
                        frame,
                        list_area,
                        &THEME,
                        &labels,
                        &mut self.list_state,
                        &title,
                    );
                }
                View::Source(slot) => {
                    let songs = self
                        .slot_tracks
                        .get(slot)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    ui::render_songs(
                        frame,
                        list_area,
                        &THEME,
                        songs,
                        current_key.as_deref(),
                        &mut self.list_state,
                        &title,
                        loading,
                        error,
                        empty,
                    );
                }
                View::Netease => match self.netease_page {
                    NeteasePage::Browse => ui::render_songs(
                        frame,
                        list_area,
                        &THEME,
                        &self.netease_chart,
                        current_key.as_deref(),
                        &mut self.list_state,
                        &title,
                        loading,
                        error,
                        empty,
                    ),
                    NeteasePage::Search => ui::render_playlists(
                        frame,
                        list_area,
                        &THEME,
                        &self.netease_playlist_hits,
                        &mut self.list_state,
                        &title,
                        loading,
                        error,
                        empty,
                    ),
                    NeteasePage::Tracks => {
                        let songs = self
                            .netease_tracks
                            .as_ref()
                            .map(|t| t.songs.as_slice())
                            .unwrap_or(&[]);
                        ui::render_songs(
                            frame,
                            list_area,
                            &THEME,
                            songs,
                            current_key.as_deref(),
                            &mut self.list_state,
                            &title,
                            loading,
                            error,
                            empty,
                        );
                    }
                },
                View::Indie => match self.indie_page {
                    IndiePage::Playlists => {
                        let playlists: Vec<Playlist> = self
                            .indie_playlists
                            .iter()
                            .map(|p| Playlist {
                                id: p.id.clone(),
                                name: p.name.clone(),
                                cover_url: p.cover_url.clone(),
                                track_count: p.songs.len() as u64,
                                play_count: 0,
                                creator: "雀乐".into(),
                                description: String::new(),
                            })
                            .collect();
                        ui::render_playlists(
                            frame,
                            list_area,
                            &THEME,
                            &playlists,
                            &mut self.list_state,
                            &title,
                            loading,
                            error,
                            empty,
                        );
                    }
                    IndiePage::Tracks => {
                        let songs = self
                            .indie_tracks
                            .as_ref()
                            .map(|t| t.songs.as_slice())
                            .unwrap_or(&[]);
                        ui::render_songs(
                            frame,
                            list_area,
                            &THEME,
                            songs,
                            current_key.as_deref(),
                            &mut self.list_state,
                            &title,
                            loading,
                            error,
                            empty,
                        );
                    }
                },
                View::StreetVoiceRecommend => ui::render_songs(
                    frame,
                    list_area,
                    &THEME,
                    &self.sv_recommend,
                    current_key.as_deref(),
                    &mut self.list_state,
                    &title,
                    loading,
                    error,
                    empty,
                ),
                View::Local => ui::render_songs(
                    frame,
                    list_area,
                    &THEME,
                    &self.local_songs,
                    current_key.as_deref(),
                    &mut self.list_state,
                    &title,
                    loading,
                    error,
                    empty,
                ),
                View::Recent => ui::render_songs(
                    frame,
                    list_area,
                    &THEME,
                    &self.history,
                    current_key.as_deref(),
                    &mut self.list_state,
                    &title,
                    loading,
                    error,
                    empty,
                ),
            }
        }

        if immersive || self.show_lyrics {
            ui::render_lyrics(
                frame,
                lyrics_area,
                &THEME,
                &self.lyrics,
                self.pos_ms,
                self.cover.as_ref().map(|(_, art)| art.as_ref()),
                &mut self.lyrics_scroll,
            );
        }

        let toast = self.toast.as_ref().and_then(|(message, at)| {
            (at.elapsed() < Duration::from_secs(4)).then_some(message.as_str())
        });
        let hint = ui::status_hint(
            toast,
            error,
            self.tag_progress
                .as_ref()
                .map(|(done, total, name)| (*done, *total, name.as_str())),
            self.cache_progress,
            self.download_progress,
        );
        ui::render_player(
            frame,
            player_area,
            &THEME,
            self.current.as_ref(),
            self.playing,
            self.loading_play,
            self.pos_ms,
            self.dur_ms,
            self.mode,
            self.sleep_deadline
                .map(|deadline| deadline.saturating_duration_since(Instant::now())),
            hint,
        );

        if self.focus == Focus::SourcePicker {
            let items: Vec<(String, String, String)> = self
                .picker_items
                .iter()
                .map(|i| {
                    (
                        i.name.clone(),
                        i.desc.clone(),
                        if i.builtin { "内置" } else { "自定义" }.to_string(),
                    )
                })
                .collect();
            let title = format!(
                "换源 · {}",
                match self.picker_slot {
                    Some(0) => "第一个入口",
                    Some(1) => "第二个入口",
                    Some(2) => "第三个入口",
                    _ => "入口",
                }
            );
            ui::render_source_picker(frame, area, &THEME, &title, &items, &mut self.picker_state);
        }
        if self.show_help {
            ui::render_help(frame, area, &THEME, &mut self.help_scroll);
        }
    }
}

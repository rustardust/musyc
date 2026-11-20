use super::*;

impl App {
    pub(super) fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.quit = true;
            return;
        }
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.help_scroll = self.help_scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.help_scroll = self.help_scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    self.help_scroll = self.help_scroll.saturating_add(5);
                }
                KeyCode::PageUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(5);
                }
                KeyCode::Char('g') | KeyCode::Home => self.help_scroll = 0,
                KeyCode::Char('G') | KeyCode::End => self.help_scroll = u16::MAX,
                _ => {}
            }
            return;
        }
        if self.focus == Focus::SourcePicker {
            self.on_picker_key(key);
            return;
        }
        if self.focus == Focus::Input {
            self.on_input_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => {
                self.show_help = true;
                self.help_scroll = 0;
            }
            KeyCode::Char('/') => {
                self.focus = Focus::Input;
                self.input_mode = InputMode::Search;
                if let Some(search) = &self.search {
                    self.input = search.query.clone();
                } else {
                    self.input.clear();
                }
                self.input_cursor = self.input.len();
            }
            KeyCode::Char('f') => {
                self.focus = Focus::Input;
                self.input_mode = InputMode::PlaylistSearch;
                self.input = self.netease_query.clone();
                self.input_cursor = self.input.len();
            }
            KeyCode::Char('o') => {
                self.focus = Focus::Input;
                self.input_mode = InputMode::Open;
                self.input.clear();
                self.input_cursor = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') | KeyCode::Home => self.select_boundary(false),
            KeyCode::Char('G') | KeyCode::End => self.select_boundary(true),
            KeyCode::PageUp => self.scroll_lyrics(-5),
            KeyCode::PageDown => self.scroll_lyrics(5),
            KeyCode::Enter => self.activate_selected(),
            KeyCode::Char(' ') => self.toggle_play(),
            KeyCode::Char(']') => self.next_song(false),
            KeyCode::Char('[') => self.previous_song(),
            KeyCode::Left => self.seek_by(-5000),
            KeyCode::Right => self.seek_by(5000),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let volume = self.volume + 0.05;
                self.set_volume(volume);
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                let volume = self.volume - 0.05;
                self.set_volume(volume);
            }
            KeyCode::Char('r') => self.cycle_mode(),
            KeyCode::Char('d') => self.download_selected(),
            KeyCode::Char('l') => {
                self.show_lyrics = !self.show_lyrics;
                // 关闭歌词时同步退出纯享模式，避免歌词区仍被纯享占据。
                if !self.show_lyrics {
                    self.lyrics_immersive = false;
                }
                self.toast(if self.show_lyrics {
                    "歌词：开"
                } else {
                    "歌词：关"
                });
            }
            KeyCode::Char('v') => {
                self.lyrics_immersive = !self.lyrics_immersive;
                // 进入纯享时强制打开歌词；退出纯享回到分栏形态。
                if self.lyrics_immersive {
                    self.show_lyrics = true;
                }
                self.toast(if self.lyrics_immersive { "纯享开" } else { "纯享关" });
            }
            KeyCode::Char('R') => self.refresh_current(),
            KeyCode::Char('t') => self.tag_selected(),
            KeyCode::Char('T') => self.tag_all_missing(),
            KeyCode::Char('s') if self.view == View::Home => self.open_source_picker(),
            KeyCode::Char('x') | KeyCode::Delete => self.remove_history_selected(),
            KeyCode::Esc => self.go_back(),
            _ => {}
        }
    }

    pub(super) fn on_input_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => {
                self.focus = Focus::List;
                if self.input_mode == InputMode::Open {
                    self.input.clear();
                    self.input_cursor = 0;
                    self.input_mode = InputMode::Search;
                }
            }
            KeyCode::Enter => self.submit_input(),
            KeyCode::Backspace => self.delete_previous(),
            KeyCode::Delete => self.delete_next(),
            KeyCode::Left => self.input_cursor = prev_boundary(&self.input, self.input_cursor),
            KeyCode::Right => self.input_cursor = next_boundary(&self.input, self.input_cursor),
            KeyCode::Home => self.input_cursor = 0,
            KeyCode::End => self.input_cursor = self.input.len(),
            KeyCode::Char('u') if ctrl => {
                self.input.clear();
                self.input_cursor = 0;
            }
            KeyCode::Char(ch) if !ctrl && !alt => {
                self.input.insert(self.input_cursor, ch);
                self.input_cursor += ch.len_utf8();
                if self.input.capacity() > 4096 {
                    self.input.shrink_to_fit();
                }
            }
            _ => {}
        }
    }
}

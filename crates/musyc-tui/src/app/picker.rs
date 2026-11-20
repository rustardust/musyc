use super::*;

impl App {
    /// 首页前三个槽位的绑定 id。
    pub(super) fn slot_ids(&self) -> [String; HOME_ONLINE_SLOTS] {
        normalize_home_entries(&self.cfg.home.entries)
    }

    /// 首页前三个槽位的显示名（用于渲染首页菜单）。
    /// 首页前三个槽位的显示名（用于渲染首页菜单）。
    pub(super) fn slot_names(&self) -> [String; HOME_ONLINE_SLOTS] {
        self.slot_ids().map(|id| self.source_name(&id))
    }

    /// 根据 id 找显示名：内置 → 内置名；自定义 → TOML 的 name；找不到 → id 本身。
    /// 根据 id 找显示名：内置 → 内置名；自定义 → TOML 的 name；找不到 → id 本身。
    pub(super) fn source_name(&self, id: &str) -> String {
        if let Some(builtin) = BUILTIN_SOURCES.iter().find(|b| b.id == id) {
            return builtin.name.to_string();
        }
        self.sources
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    pub(super) fn slot_label(&self, slot: usize) -> &'static str {
        match slot {
            0 => "第一个入口",
            1 => "第二个入口",
            2 => "第三个入口",
            _ => "入口",
        }
    }

    /// 首页五个入口的显示名（前三个是自定义/内置绑定，后两个固定）。
    /// 首页五个入口的显示名（前三个是自定义/内置绑定，后两个固定）。
    pub(super) fn home_labels(&self) -> [String; 5] {
        let names = self.slot_names();
        [
            names[0].clone(),
            names[1].clone(),
            names[2].clone(),
            "本地列表".to_string(),
            "最近播放".to_string(),
        ]
    }

    /// 解析首页槽位绑定的 Source（自定义源）；内置 id 返回 None。
    /// 解析首页槽位绑定的 Source（自定义源）；内置 id 返回 None。
    pub(super) fn slot_source(&self, slot: usize) -> Option<Source> {
        let id = home_entry_id(&self.cfg.home.entries, slot);
        self.sources.iter().find(|s| s.id == id).cloned()
    }

    // ---------- 视图切换与加载 ----------

    /// 打开首页槽位的换源浮层；前三个位才可换，后两个固定。
    pub(super) fn open_source_picker(&mut self) {
        let index = match self.list_state.selected() {
            Some(i) => i,
            None => {
                self.toast("请选在线");
                return;
            }
        };
        if index >= HOME_ONLINE_SLOTS {
            self.toast("本地不可换");
            return;
        }
        let mut items: Vec<SourcePickerItem> = BUILTIN_SOURCES
            .iter()
            .map(|b| SourcePickerItem {
                id: b.id.to_string(),
                name: b.name.to_string(),
                desc: b.desc.to_string(),
                builtin: true,
            })
            .collect();
        for source in &self.sources {
            items.push(SourcePickerItem {
                id: source.id.clone(),
                name: source.name.clone(),
                desc: source.desc.clone(),
                builtin: false,
            });
        }
        let current = home_entry_id(&self.cfg.home.entries, index);
        let selected = items.iter().position(|i| i.id == current).unwrap_or(0);
        self.picker_items = items;
        self.picker_slot = Some(index);
        self.picker_state.select(Some(selected));
        self.focus = Focus::SourcePicker;
    }

    pub(super) fn on_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.focus = Focus::List;
                self.picker_slot = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.picker_items.len();
                if len == 0 {
                    return;
                }
                let cur = self.picker_state.selected().unwrap_or(0);
                self.picker_state.select(Some((cur + 1).min(len - 1)));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let cur = self.picker_state.selected().unwrap_or(0);
                self.picker_state.select(Some(cur.saturating_sub(1)));
            }
            KeyCode::Char('g') | KeyCode::Home => self.picker_state.select(Some(0)),
            KeyCode::Char('G') | KeyCode::End => {
                let len = self.picker_items.len();
                if len > 0 {
                    self.picker_state.select(Some(len - 1));
                }
            }
            KeyCode::Enter => self.apply_source_pick(),
            _ => {}
        }
    }

    pub(super) fn apply_source_pick(&mut self) {
        let Some(slot) = self.picker_slot else { return };
        let Some(index) = self.picker_state.selected() else {
            return;
        };
        let Some(item) = self.picker_items.get(index).cloned() else {
            return;
        };
        // 同 id 选中时也允许直接确认（等价于 no-op）
        let mut entries = self.cfg.home.entries.clone();
        while entries.len() < HOME_ONLINE_SLOTS {
            entries.push(String::new());
        }
        entries[slot] = item.id.clone();
        self.cfg.home.entries = entries;
        // 持久化
        if let Err(e) = self.cfg.save(&self.paths.config_file) {
            self.toast("保存失败");
        }
        // 使该槽位失效并重新拉取
        self.slot_loaded[slot] = false;
        self.slot_loading[slot] = false;
        self.slot_error[slot] = None;
        self.slot_tracks[slot].clear();
        self.selection_memory.insert(ListKey::Source(slot), 0);
        self.focus = Focus::List;
        self.picker_slot = None;
        self.ensure_slot(slot);
        self.toast("已绑定");
    }
}

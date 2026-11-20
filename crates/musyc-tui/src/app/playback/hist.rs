use super::*;

impl App {
    pub(in crate::app) fn record_history(&mut self, song: &Song) {
        if let Err(e) = history::append(&self.paths.history_file, song) {
            tracing::warn!("写入历史失败: {e}");
        }
        self.history = history::recent(&self.paths.history_file, 50);
    }

    /// 仅当列表展示的是“最近播放”时允许删除。
    /// 仅当列表展示的是“最近播放”时允许删除。
    pub(in crate::app) fn remove_history_selected(&mut self) {
        if self.view != View::Recent || self.search.is_some() {
            self.toast("只能删");
            return;
        }
        let Some(index) = self.selected_index() else {
            return;
        };
        let Some(song) = self.history.get(index).cloned() else {
            return;
        };
        match history::remove(&self.paths.history_file, &song.key()) {
            Ok(0) => self.toast("记录已不存在"),
            Ok(_) => {
                self.history = history::recent(&self.paths.history_file, 50);
                self.ensure_selection();
                self.toast("已移除");
            }
            Err(_e) => self.toast("删失败"),
        }
    }
}

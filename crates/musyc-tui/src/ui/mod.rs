//! UI 渲染拆分：首页 / 列表 / 歌单 / 搜索 / 歌词 / 播放器 / 帮助 / 换源。
//! 对外保持 `crate::ui::render_*` 路径不变。

mod help;
mod home;
mod list;
mod lyrics;
mod picker;
mod player;
mod playlist;
mod search;

pub use help::render_help;
pub use home::{block, render_home, render_too_small};
pub use list::render_songs;
pub use lyrics::render_lyrics;
pub use picker::render_source_picker;
pub use player::{render_player, status_hint};
pub use playlist::render_playlists;
pub use search::render_search;

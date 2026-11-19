use musyc_core::model::PlayMode;

/// 单一首页列表中的入口；选中后进入对应内容列表。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Home,
    Netease,
    StreetVoiceRecommend,
    Indie,
    Local,
    Recent,
    /// 首页前三个在线槽位之一的内容列表（参数=槽位序号 0..3）。
    Source(usize),
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Home => "首页",
            View::Netease => "网易原创",
            View::StreetVoiceRecommend => "街声推荐",
            View::Indie => "雀乐推荐",
            View::Local => "本地列表",
            View::Recent => "最近播放",
            View::Source(_) => "自定义源",
        }
    }
}

/// 首页五个入口：前三个可通过 `s` 键换绑 sources/*.toml（或内置 id），
/// 后两个固定为本地列表 / 最近播放。
pub const HOME_ONLINE_SLOTS: usize = 3;

/// 解析首页第 `index` 个入口的绑定 id（0..3）。读取 `[home].entries`，缺位时用默认。
pub fn home_entry_id(entries: &[String], index: usize) -> String {
    const DEFAULTS: [&str; 3] = ["netease", "streetvoice", "indie"];
    entries
        .get(index)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULTS[index.min(2)].to_string())
}

/// 归一化：始终返回 3 个绑定 id（用于保存前规整）。
pub fn normalize_home_entries(entries: &[String]) -> [String; 3] {
    [
        home_entry_id(entries, 0),
        home_entry_id(entries, 1),
        home_entry_id(entries, 2),
    ]
}

/// 换源选择器里的一行。
#[derive(Debug, Clone)]
pub struct SourcePickerItem {
    /// 源的 id（保存到 [home].entries）。
    pub id: String,
    /// 显示名。
    pub name: String,
    /// 简短描述。
    pub desc: String,
    /// 是否内置源。
    pub builtin: bool,
}

/// 键盘焦点：输入框或列表。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    List,
    Input,
    /// 首页换源选择器浮层（前三个槽位按 s 弹出）。
    SourcePicker,
}

/// 输入框当前用途，只影响标题与提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Search,
    PlaylistSearch,
    Open,
}

impl InputMode {
    pub fn label(self) -> &'static str {
        match self {
            InputMode::Search => "搜索单曲",
            InputMode::PlaylistSearch => "搜索歌单",
            InputMode::Open => "打开",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            InputMode::Search => "歌曲关键词…",
            InputMode::PlaylistSearch => "网易云歌单关键词…",
            InputMode::Open => "路径 / 链接 / ID / M3U…",
        }
    }
}

/// 解码后的封面缩略图，供像素风渲染使用。
#[derive(Debug, Clone)]
pub struct CoverArt {
    pub width: u32,
    pub height: u32,
    /// RGB8，长度始终为 `width * height * 3`。
    pub rgb: Vec<u8>,
}

/// MPRIS 回调投递到主循环的动作。
#[derive(Debug, Clone)]
pub enum Action {
    TogglePlay,
    Next,
    Previous,
    SeekDelta(i64),
    SeekTo(u64),
    SetVolume(f32),
    SetMode(PlayMode),
    SetShuffle(bool),
}

use super::*;
#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct SessionFile {
    pub(super) queue: Vec<Song>,
    pub(super) index: Option<usize>,
    pub(super) pos_ms: u64,
}

#[derive(Debug, Clone)]
pub(super) struct Opened {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) songs: Vec<Song>,
}

#[derive(Debug, Clone)]
pub(super) struct SearchResults {
    pub(super) query: String,
    pub(super) songs: Vec<Song>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum NeteasePage {
    #[default]
    Browse,
    Search,
    Tracks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum IndiePage {
    #[default]
    Playlists,
    Tracks,
}

/// 每个列表在 `selection_memory` 中使用的键；歌单用 ID 区分，返回时能回到原位。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ListKey {
    Home,
    NeteaseBrowse,
    NeteaseSearch,
    NeteaseTracks(String),
    IndiePlaylists,
    IndieTracks(String),
    StreetVoiceRecommend,
    Source(usize),
    Local,
    Recent,
    Search,
}

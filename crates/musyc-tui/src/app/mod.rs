use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use ferrous_opencc::OpenCC;
use ferrous_opencc::config::BuiltinConfig;

use musyc_api::{ApiClient, IndiePlaylist, RawLyric, Resolver, load_custom_source};
use musyc_core::config::Config;
use musyc_core::lyric::Lyrics;
use musyc_core::model::{PlayMode, Playlist, Quality, Song, SourceKind};
use musyc_core::sources::{BUILTIN_SOURCES, Source};
use musyc_core::{Error, Paths, Result};
use musyc_player::{AudioCache, AudioEngine, Cmd, PlayerEvent};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::TableState;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::event::{Event, LocalScan, TagUpdate};
use crate::fallback;
use crate::history;
use crate::input::{self, Classified};
use crate::m3u;
use crate::mpris::{self, MprisCmd, MprisLoop, MprisState};
use crate::theme::THEME;
use crate::types::{
    Action, CoverArt, Focus, HOME_ONLINE_SLOTS, InputMode, SourcePickerItem, View, home_entry_id,
    normalize_home_entries,
};
use crate::ui;

/// 低于这个尺寸时只显示一行提示，避免表格 / 歌词互相挤压成乱码。
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 20;

pub struct App {
    cfg: Config,
    paths: Paths,
    api: Arc<ApiClient>,
    resolver: Arc<Resolver>,
    http: reqwest::Client,
    cache: AudioCache,
    engine: AudioEngine,
    tx: UnboundedSender<Event>,

    // 当前视图与输入
    view: View,
    input: String,
    input_cursor: usize,
    input_mode: InputMode,
    focus: Focus,
    list_state: TableState,
    /// 每个列表最后停留的位置，切换视图 / 返回时恢复。
    selection_memory: HashMap<ListKey, usize>,
    error: Option<String>,

    // 全局单曲搜索
    search: Option<SearchResults>,
    search_seq: u64,
    search_loading: bool,

    // 网易云
    netease_chart: Vec<Song>,
    netease_chart_seq: u64,
    netease_chart_loading: bool,
    netease_chart_loaded: bool,
    netease_playlist_hits: Vec<Playlist>,
    netease_tracks: Option<Opened>,
    netease_page: NeteasePage,
    netease_query: String,
    netease_seq: u64,
    netease_loading: bool,
    /// 打开歌单时是否来自歌单搜索结果，决定 Esc 返回到哪里。
    netease_back_to_search: bool,

    // 雀乐
    indie_playlists: Vec<IndiePlaylist>,
    indie_tracks: Option<Opened>,
    indie_page: IndiePage,
    indie_seq: u64,
    indie_loading: bool,
    indie_loaded: bool,

    // 街声
    sv_recommend: Vec<Song>,
    sv_recommend_seq: u64,
    sv_recommend_loading: bool,
    sv_recommend_loaded: bool,

    // 自定义源（首页前三个入口可绑 sources/*.toml）
    sources: Vec<Source>,
    /// 每个前三个槽位的内容缓存（按槽位序号 0..3 索引）。
    slot_tracks: [Vec<Song>; HOME_ONLINE_SLOTS],
    slot_loading: [bool; HOME_ONLINE_SLOTS],
    slot_loaded: [bool; HOME_ONLINE_SLOTS],
    slot_seq: [u64; HOME_ONLINE_SLOTS],
    slot_error: [Option<String>; HOME_ONLINE_SLOTS],
    /// 换源选择器：当前弹出的是哪个槽位（0..3），以及列表指针。
    picker_slot: Option<usize>,
    picker_items: Vec<SourcePickerItem>,
    picker_state: TableState,

    // 本地
    local_songs: Vec<Song>,
    local_needs_tag: HashSet<String>,
    local_seq: u64,
    local_loading: bool,
    local_loaded: bool,

    // 最近播放
    history: Vec<Song>,

    // 打开 M3U / 链接
    open_seq: u64,

    // 播放
    queue: Vec<Song>,
    cursor: Option<usize>,
    current: Option<Song>,
    loaded: bool,
    playing: bool,
    loading_play: bool,
    pos_ms: u64,
    dur_ms: u64,
    volume: f32,
    quality: Quality,
    mode: PlayMode,
    play_seq: u64,
    next_start_ms: u64,
    pending_autoplay: bool,
    /// 连续播放失败次数，达到队列长度或上限后停止，避免无限跳过。
    failed_streak: u32,
    cache_progress: Option<(u64, Option<u64>)>,

    // 歌词
    lyrics: Lyrics,
    lyrics_key: Option<String>,
    show_lyrics: bool,
    /// 歌词纯享模式：歌词面板向上拓展到整个内容区（列表隐藏）。
    lyrics_immersive: bool,
    /// 相对自动滚动位置的偏移，PageUp / PageDown 调整。
    lyrics_scroll: i32,

    // 封面
    cover: Option<(String, Arc<CoverArt>)>,
    cover_cache: HashMap<String, Arc<CoverArt>>,
    cover_seq: u64,

    // 下载
    download_seq: u64,
    downloading: Option<(u64, String)>,
    download_progress: Option<(u64, Option<u64>)>,

    // 本地标签补全
    tag_seq: u64,
    tagging: bool,
    tag_progress: Option<(u64, u64, String)>,

    // 界面状态
    show_help: bool,
    help_scroll: u16,
    toast: Option<(String, Instant)>,
    quit: bool,

    // MPRIS
    mpris_cmd: Option<UnboundedSender<MprisCmd>>,
    mpris_last_sync: Instant,
}

mod convert;
mod cover;
mod download;
mod events;
mod keys;
mod loading;
mod matching;
mod navigation;
mod picker;
mod playback;
mod render;
mod scan;
mod search;
mod session;
mod state;
mod tags;

use self::convert::*;
use self::matching::*;
use self::scan::*;
use self::session::load_session;
use self::state::*;

impl App {
    pub fn new(
        cfg: Config,
        paths: Paths,
        initial: Option<String>,
    ) -> Result<(Self, UnboundedReceiver<Event>)> {
        let api = Arc::new(ApiClient::new(&cfg.api)?);
        let resolver = Arc::new(Resolver::new(api.clone()));
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent("Mozilla/5.0 musyc/0.1")
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        let cache = AudioCache::new(paths.audio_cache.clone(), cfg.player.cache_limit_mb);
        cache.clean_partials();
        let engine = AudioEngine::spawn(cfg.player.volume);
        let history_songs = history::recent(&paths.history_file, 50);

        let session = if cfg.ui.restore_session {
            load_session(&paths.session_file)
        } else {
            None
        };
        let mut queue = Vec::new();
        let mut cursor = None;
        let mut current = None;
        let mut pos_ms = 0u64;
        let mut dur_ms = 0u64;
        if let Some(session) = session
            && !session.queue.is_empty()
        {
            let index = session.index.unwrap_or(0).min(session.queue.len() - 1);
            queue = session.queue;
            cursor = Some(index);
            current = queue.get(index).cloned();
            dur_ms = current.as_ref().map(|s| s.duration_ms).unwrap_or(0);
            pos_ms = if dur_ms > 0 {
                session.pos_ms.min(dur_ms)
            } else {
                session.pos_ms
            };
        }

        let mut list_state = TableState::default();
        list_state.select(Some(0));

        let volume = cfg.player.volume;
        let quality = cfg.player.quality;
        let mode = cfg.player.mode;
        let (tx, rx) = mpsc::unbounded_channel();

        let mut app = Self {
            cfg,
            paths,
            api,
            resolver,
            http,
            cache,
            engine,
            tx,
            view: View::default(),
            input: String::new(),
            input_cursor: 0,
            input_mode: InputMode::Search,
            focus: Focus::List,
            list_state,
            selection_memory: HashMap::new(),
            error: None,
            search: None,
            search_seq: 0,
            search_loading: false,
            netease_chart: Vec::new(),
            netease_chart_seq: 0,
            netease_chart_loading: false,
            netease_chart_loaded: false,
            netease_playlist_hits: Vec::new(),
            netease_tracks: None,
            netease_page: NeteasePage::default(),
            netease_query: String::new(),
            netease_seq: 0,
            netease_loading: false,
            netease_back_to_search: false,
            indie_playlists: Vec::new(),
            indie_tracks: None,
            indie_page: IndiePage::default(),
            indie_seq: 0,
            indie_loading: false,
            indie_loaded: false,
            sv_recommend: Vec::new(),
            sv_recommend_seq: 0,
            sv_recommend_loading: false,
            sv_recommend_loaded: false,
            sources: Vec::new(),
            slot_tracks: [Vec::new(), Vec::new(), Vec::new()],
            slot_loading: [false; HOME_ONLINE_SLOTS],
            slot_loaded: [false; HOME_ONLINE_SLOTS],
            slot_seq: [0; HOME_ONLINE_SLOTS],
            slot_error: [None, None, None],
            picker_slot: None,
            picker_items: Vec::new(),
            picker_state: TableState::default(),
            local_songs: Vec::new(),
            local_needs_tag: HashSet::new(),
            local_seq: 0,
            local_loading: false,
            local_loaded: false,
            history: history_songs,
            open_seq: 0,
            queue,
            cursor,
            current,
            loaded: false,
            playing: false,
            loading_play: false,
            pos_ms,
            dur_ms,
            volume,
            quality,
            mode,
            play_seq: 0,
            next_start_ms: 0,
            pending_autoplay: true,
            failed_streak: 0,
            cache_progress: None,
            lyrics: Lyrics::default(),
            lyrics_key: None,
            show_lyrics: false,
            lyrics_immersive: false,
            lyrics_scroll: 0,
            cover: None,
            cover_cache: HashMap::new(),
            cover_seq: 0,
            download_seq: 0,
            downloading: None,
            download_progress: None,
            tag_seq: 0,
            tagging: false,
            tag_progress: None,
            show_help: false,
            help_scroll: 0,
            toast: None,
            quit: false,
            mpris_cmd: None,
            mpris_last_sync: Instant::now(),
        };

        app.check_api();
        if app.cfg.ui.mpris {
            app.start_mpris();
        }
        app.sources = musyc_core::sources::discover(&app.paths);
        for slot in 0..HOME_ONLINE_SLOTS {
            let bound = home_entry_id(&app.cfg.home.entries, slot);
            if bound == musyc_core::sources::BUILTIN_NETEASE
                || bound == musyc_core::sources::BUILTIN_STREETVOICE
                || bound == musyc_core::sources::BUILTIN_INDIE
            {
                continue;
            }
            // 自定义源：启动时提示，但不在未打开时强拉取。
            let _ = bound;
        }
        app.ensure_home_data();
        // 歌词面板默认打开；恢复会话时也要把当前歌曲的歌词与封面取回来。
        if let Some(song) = app.current.clone() {
            app.load_lyrics(&song);
            app.load_cover(&song);
        }
        if let Some(raw) = initial {
            app.submit_raw(&raw);
        }
        Ok((app, rx))
    }

    pub fn sender(&self) -> UnboundedSender<Event> {
        self.tx.clone()
    }

    pub fn quit(&self) -> bool {
        self.quit
    }

    pub fn tick(&mut self) {
        while let Some(event) = self.engine.try_recv() {
            self.on_player(event);
        }
        if let Some((_, at)) = &self.toast
            && at.elapsed() > Duration::from_secs(4)
        {
            self.toast = None;
        }
        self.ensure_selection();
        self.sync_mpris(false);
    }

    // ---------- 键盘 ----------

    // ---------- MPRIS / 配置 ----------

    pub fn finish(&mut self) {
        self.cfg.player.volume = self.volume;
        self.cfg.player.quality = self.quality;
        self.cfg.player.mode = self.mode;
        if let Err(e) = self.cfg.save(&self.paths.config_file) {
            tracing::warn!("保存配置失败: {e}");
        }
        self.save_session();
    }

    fn toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }
}

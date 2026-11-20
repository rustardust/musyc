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

#[derive(Debug, Default, Serialize, Deserialize)]
struct SessionFile {
    queue: Vec<Song>,
    index: Option<usize>,
    pos_ms: u64,
}

#[derive(Debug, Clone)]
struct Opened {
    id: String,
    title: String,
    songs: Vec<Song>,
}

#[derive(Debug, Clone)]
struct SearchResults {
    query: String,
    songs: Vec<Song>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum NeteasePage {
    #[default]
    Browse,
    Search,
    Tracks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum IndiePage {
    #[default]
    Playlists,
    Tracks,
}

/// 每个列表在 `selection_memory` 中使用的键；歌单用 ID 区分，返回时能回到原位。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ListKey {
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

mod keys;
mod loaders;
mod navigation;
mod picker;
mod playback;

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

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.on_key(key),
            Event::Resize => {}
            Event::SearchSongs {
                seq,
                query,
                autoplay,
                result,
            } => {
                if seq != self.search_seq {
                    return;
                }
                self.search_loading = false;
                match result {
                    Ok(songs) => {
                        if autoplay && !songs.is_empty() {
                            self.search = Some(SearchResults {
                                query: query.clone(),
                                songs: songs.clone(),
                            });
                            self.queue = songs;
                            self.play_at(0, 0, true);
                            self.list_state.select(Some(0));
                        } else {
                            let empty = songs.is_empty();
                            self.search = Some(SearchResults { query, songs });
                            self.list_state.select(Some(0));
                            self.error = None;
                            if empty {
                                self.toast("没有找到歌曲");
                            }
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("搜索失败");
                    }
                }
                self.ensure_selection();
            }
            Event::OpenSongs { seq, title, result } => {
                if seq != self.open_seq {
                    return;
                }
                match result {
                    Ok(songs) if !songs.is_empty() => self.adopt_queue(title, songs),
                    Ok(_) => self.toast("无可播放"),
                    Err(_) => self.toast("打开失败"),
                }
            }
            Event::HomeSlot { seq, slot, result } => {
                if slot >= HOME_ONLINE_SLOTS {
                    return;
                }
                if seq != self.slot_seq[slot] {
                    return;
                }
                self.slot_loading[slot] = false;
                match result {
                    Ok(songs) => {
                        self.slot_tracks[slot] = songs;
                        self.slot_loaded[slot] = true;
                        self.slot_error[slot] = None;
                    }
                    Err(e) => {
                        self.slot_error[slot] = Some(e.clone());
                        self.toast("加载失败");
                    }
                }
                self.ensure_selection();
            }
            Event::NeteaseChart { seq, result } => {
                if seq != self.netease_chart_seq {
                    return;
                }
                self.netease_chart_loading = false;
                match result {
                    Ok(songs) => {
                        self.netease_chart = songs;
                        self.netease_chart_loaded = true;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("加载失败");
                    }
                }
                self.ensure_selection();
            }
            Event::NeteasePlaylistHits { seq, query, result } => {
                if seq != self.netease_seq {
                    return;
                }
                self.netease_loading = false;
                match result {
                    Ok(playlists) => {
                        let empty = playlists.is_empty();
                        self.netease_playlist_hits = playlists;
                        self.netease_query = query;
                        self.error = None;
                        if empty {
                            self.toast("没有找到歌单");
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("搜索失败");
                    }
                }
                self.ensure_selection();
            }
            Event::NeteaseTracks {
                seq,
                id,
                title,
                result,
            } => {
                if seq != self.netease_seq {
                    return;
                }
                self.netease_loading = false;
                match result {
                    Ok(songs) => {
                        let empty = songs.is_empty();
                        self.netease_tracks = Some(Opened { id, title, songs });
                        self.error = None;
                        if empty {
                            self.toast("歌单为空");
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("打开失败");
                    }
                }
                self.ensure_selection();
            }
            Event::IndieCatalog { seq, result } => {
                if seq != self.indie_seq {
                    return;
                }
                self.indie_loading = false;
                match result {
                    Ok(playlists) => {
                        self.indie_playlists = playlists;
                        self.indie_loaded = true;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("加载失败");
                    }
                }
                self.ensure_selection();
            }
            Event::StreetVoiceRecommend { seq, result } => {
                if seq != self.sv_recommend_seq {
                    return;
                }
                self.sv_recommend_loading = false;
                match result {
                    Ok(songs) => {
                        self.sv_recommend = songs;
                        self.sv_recommend_loaded = true;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("加载失败");
                    }
                }
                self.ensure_selection();
            }
            Event::LocalSongs { seq, result } => {
                if seq != self.local_seq {
                    return;
                }
                self.local_loading = false;
                match result {
                    Ok(scan) => {
                        self.local_songs = scan.songs;
                        self.local_needs_tag = scan.needs_tag.into_iter().collect();
                        self.local_loaded = true;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.toast("扫描失败");
                    }
                }
                self.ensure_selection();
            }
            Event::TagResult {
                seq,
                old_id,
                updated,
                message,
            } => {
                if seq != self.tag_seq {
                    return;
                }
                self.tagging = false;
                self.tag_progress = None;
                if let Some(update) = updated {
                    self.local_needs_tag.remove(&old_id);
                    if let Some(song) = self.local_songs.iter_mut().find(|s| s.id == old_id) {
                        song.id = update.id.clone();
                        song.origin = update.origin.clone();
                        song.name = update.title.clone();
                        song.artists = update.artists.clone();
                        song.album = update.album.clone();
                    }
                    if let Some(current) = &mut self.current
                        && current.id == old_id
                    {
                        current.id = update.id.clone();
                        current.origin = update.origin.clone();
                        current.name = update.title;
                        current.artists = update.artists;
                        current.album = update.album;
                        let current = current.clone();
                        // 正在播放的原文件刚刚被补全写入正文，歌词/封面只存在于
                        // 新生成的转换副本里，立即重读，歌词面板即时出现歌词。
                        self.load_lyrics(&current);
                        let old_key = format!("本地:{old_id}");
                        self.cover_cache.remove(&old_key);
                        self.load_cover(&current);
                    }
                    self.toast(message);
                    // 标签写入正文后立刻重扫本地，补全结果即时显示。
                    self.scan_local();
                } else {
                    self.toast(message);
                }
            }
            Event::TagProgress {
                seq,
                done,
                total,
                name,
            } => {
                if seq == self.tag_seq {
                    self.tag_progress = Some((done, total, name));
                }
            }
            Event::TagDone { seq, .. } => {
                if seq != self.tag_seq {
                    return;
                }
                self.tagging = false;
                self.tag_progress = None;
                self.toast("补全完成");
                self.scan_local();
            }
            Event::Lyric { key, raw } => {
                let is_current = self.current.as_ref().is_some_and(|s| s.key() == key);
                if is_current && self.lyrics_key.as_deref() == Some(key.as_str()) {
                    self.lyrics = Lyrics::parse(&raw.lrc, &raw.tlyric);
                }
            }
            Event::Cover { key, result } => {
                if self.current.as_ref().is_some_and(|s| s.key() == key) {
                    match result {
                        Ok(art) => {
                            let art = Arc::new(art);
                            self.cover_cache.insert(key.clone(), art.clone());
                            self.cover = Some((key, art));
                        }
                        Err(_) => self.cover = None,
                    }
                }
            }
            Event::ApiStatus { ok, message: _ } => {
                if !ok {
                    self.toast("接口异常");
                }
            }
            Event::Resolved {
                seq,
                song,
                duration_ms,
                preview,
                ..
            } => {
                if seq != self.play_seq {
                    return;
                }
                let key = song.key();
                self.patch_song(
                    &key,
                    duration_ms.filter(|d| *d > 0),
                    Some(song.album.as_str()).filter(|a| !a.trim().is_empty()),
                );
                if preview {
                    self.toast("仅获取到试听片段");
                }
                self.cache_progress = Some((0, None));
            }
            Event::CacheProgress { seq, done, total } => {
                if seq != self.play_seq {
                    return;
                }
                self.cache_progress = Some((done, total));
            }
            Event::CacheReady {
                seq,
                song,
                path,
                provider: _,
            } => {
                if seq != self.play_seq {
                    return;
                }
                let start = self.next_start_ms;
                let autoplay = self.pending_autoplay;
                self.next_start_ms = 0;
                self.loading_play = false;
                self.loaded = true;
                self.playing = autoplay;
                self.cache_progress = None;
                self.engine.send(Cmd::Load {
                    path,
                    duration_ms: song.duration_ms.max(self.dur_ms),
                    start_ms: start,
                    autoplay,
                });
                if autoplay {
                    self.record_history(&song);
                }
                self.sync_mpris(true);
            }
            Event::PlayError { seq, message } => {
                if seq != self.play_seq {
                    return;
                }
                self.skip_unplayable(message);
            }
            Event::DownloadProgress { seq, done, total } => {
                if seq != self.download_seq || self.downloading.is_none() {
                    return;
                }
                self.download_progress = Some((done, total));
            }
            Event::DownloadDone { seq, .. } => {
                if seq != self.download_seq {
                    return;
                }
                self.downloading = None;
                self.download_progress = None;
                self.toast("下载完成");
            }
            Event::DownloadFailed { seq, .. } => {
                if seq != self.download_seq {
                    return;
                }
                self.downloading = None;
                self.download_progress = None;
                self.toast("下载失败");
            }
            Event::Player(event) => self.on_player(event),
            Event::Action(action) => self.handle_action(action),
        }
    }

    fn on_player(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::Ready => {
                // 解码成功才清零；连续坏文件必须在达到上限后停下。
                self.loading_play = false;
                self.failed_streak = 0;
            }
            PlayerEvent::Ended => {
                self.pos_ms = self.dur_ms;
                self.next_song(true);
            }
            PlayerEvent::Error(message) => {
                self.skip_unplayable(message);
            }
            PlayerEvent::Position {
                pos_ms,
                dur_ms,
                playing,
            } => {
                // 加载新曲期间，引擎可能还在上报上一首的进度，忽略。
                if self.loading_play {
                    return;
                }
                self.pos_ms = pos_ms;
                if dur_ms > 0 {
                    self.dur_ms = dur_ms;
                    // 播放器拿到了真实时长：回写到当前歌曲和所有包含它的列表，
                    // 让列表里原本显示 --:-- 的时长变成实际值。
                    if self
                        .current
                        .as_ref()
                        .is_some_and(|song| song.duration_ms == 0)
                    {
                        let key = self.current.as_ref().map(|s| s.key());
                        if let Some(key) = key {
                            self.patch_song(&key, Some(dur_ms), None);
                        }
                    }
                }
                self.playing = playing;
            }
        }
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::TogglePlay => self.toggle_play(),
            Action::Next => self.next_song(false),
            Action::Previous => self.previous_song(),
            Action::SeekDelta(delta) => self.seek_by(delta),
            Action::SeekTo(ms) => self.seek_to(ms),
            Action::SetVolume(volume) => self.set_volume(volume),
            Action::SetMode(mode) => self.set_mode(mode),
            Action::SetShuffle(shuffle) => self.set_shuffle(shuffle),
        }
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

    // ---------- MPRIS / 配置 ----------

    fn start_mpris(&mut self) {
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

    fn sync_mpris(&mut self, force: bool) {
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

    fn save_session(&self) {
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

fn load_session(path: &Path) -> Option<SessionFile> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn local_origin(song: &Song) -> PathBuf {
    song.origin
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&song.id))
}

fn same_dir(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// 复制原始文件到转换目录，写入标签 / 歌词 / 封面，再原子改名。
/// 原始文件始终保持不动。
async fn convert_and_tag(
    api: &ApiClient,
    http: &reqwest::Client,
    local: &Song,
    matched: &Song,
    source_dir: &Path,
    output_dir: &Path,
) -> std::result::Result<TagUpdate, String> {
    let lyric = match api.lyric(&matched.id).await {
        Ok(raw) if !raw.lrc.trim().is_empty() => Some(raw.lrc),
        _ => {
            fallback::lrclib_lyrics(
                http,
                &matched.name,
                &matched.artist_text(),
                matched.duration_ms,
            )
            .await
        }
    };
    let cover = match matched
        .cover_url
        .as_deref()
        .filter(|u| !u.trim().is_empty())
    {
        Some(url) => match http
            .get(url)
            .header("Referer", "https://music.163.com/")
            .send()
            .await
        {
            Ok(resp) => resp
                .bytes()
                .await
                .ok()
                .map(|bytes| bytes.to_vec())
                .filter(|bytes| !bytes.is_empty()),
            Err(_) => None,
        },
        None => None,
    };

    write_converted(
        local,
        ConvertCtx {
            source_dir,
            output_dir,
        },
        &matched.name,
        &matched.artists,
        &matched.album,
        lyric,
        cover,
    )
    .await
}

/// 外部回退准备：元数据走 MusicBrainz + Cover Art Archive，歌词走 LRCLIB。
/// 返回写文件所需的全部零件，由公共核心落盘。
pub struct FallbackParts {
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub lyric: Option<String>,
    pub cover: Option<Vec<u8>>,
}

async fn prepare_fallback(
    http: &reqwest::Client,
    local: &Song,
    hint_artist: Option<&str>,
    hint_title: Option<&str>,
) -> std::result::Result<FallbackParts, String> {
    let origin = local_origin(local);
    let (file_artist, file_title) = musyc_library::filename_hint(&origin);
    let title = hint_title
        .map(str::to_string)
        .or(file_title)
        .map(|s| musyc_library::clean_match_title(s.trim()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| musyc_library::clean_match_title(&local.name));
    let artist = hint_artist
        .map(str::to_string)
        .or(file_artist)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let meta = fallback::musicbrainz_meta(http, &title, &artist, local.duration_ms)
        .await
        .ok_or_else(|| format!("外部库也没找到匹配（{title} {artist}）"))?;
    let lyric = fallback::lrclib_lyrics(http, &title, &artist, local.duration_ms).await;
    Ok(FallbackParts {
        title: meta.title,
        artists: meta.artists,
        album: meta.album,
        lyric,
        cover: meta.cover,
    })
}

/// 转换用的路径对。
#[derive(Clone, Copy)]
struct ConvertCtx<'a> {
    source_dir: &'a Path,
    output_dir: &'a Path,
}

/// 补全流程用到的三个客户端：网易云 API、网易云专用 HTTP、外部库（直连、无代理）。
struct ConvertClients<'a> {
    api: &'a ApiClient,
    http: &'a reqwest::Client,
    web: &'a reqwest::Client,
}

/// 单首 / 批量共用的补全流程：网易云搜索 → pick_best_match → convert_and_tag；
/// 未命中或转换失败时走 MusicBrainz/LRCLIB 外部回退。
/// 全部失败只返回 Err(原因)，由调用方决定 log_failure 与计数。
async fn resolve_and_convert(
    client: ConvertClients<'_>,
    song: &Song,
    hint_artist: Option<&str>,
    hint_title: Option<&str>,
    query: &str,
    ctx: ConvertCtx<'_>,
) -> std::result::Result<TagUpdate, String> {
    let ConvertClients { api, http, web } = client;
    let netease_err = match api.search_songs(query, 15, 0).await {
        Ok(candidates) => match pick_best_match(song, hint_artist, hint_title, &candidates) {
            Some(matched) => {
                match convert_and_tag(api, http, song, &matched, ctx.source_dir, ctx.output_dir)
                    .await
                {
                    Ok(update) => return Ok(update),
                    Err(e) => format!("网易云转换失败：{e}"),
                }
            }
            None => "网易云未找到匹配".to_string(),
        },
        Err(e) => format!("网易云搜索失败：{e}"),
    };
    match prepare_fallback(web, song, hint_artist, hint_title).await {
        Ok(parts) => write_converted(
            song,
            ctx,
            &parts.title,
            &parts.artists,
            &parts.album,
            parts.lyric,
            parts.cover,
        )
        .await
        .map_err(|e| format!("{netease_err}；外部库转换失败：{e}")),
        Err(e) => Err(format!("{netease_err}；外部库：{e}")),
    }
}

/// 公共核心：复制原始文件、写标签 / 歌词 / 封面、原子改名，
/// 返回界面用的 [`TagUpdate`]。网易云和外部回退都走这里。
async fn write_converted(
    local: &Song,
    ctx: ConvertCtx<'_>,
    title: &str,
    artists: &[String],
    album: &str,
    lyric: Option<String>,
    cover: Option<Vec<u8>>,
) -> std::result::Result<TagUpdate, String> {
    let ConvertCtx {
        source_dir,
        output_dir,
    } = ctx;
    if same_dir(source_dir, output_dir) {
        return Err("转换目录不能与源目录相同，请修改配置 [local] output_dir".into());
    }
    let origin = local_origin(local);
    if !origin.exists() {
        return Err(format!("原始文件不存在：{}", origin.display()));
    }
    if origin.starts_with(output_dir) {
        return Err("该文件已经在转换目录里".into());
    }
    let relative = origin.strip_prefix(source_dir).map_or_else(
        |_| {
            origin
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("unknown"))
        },
        Path::to_path_buf,
    );
    let dest = output_dir.join(relative);

    let title = title.to_string();
    let artists = artists.to_vec();
    let album = album.to_string();
    let dest_label = dest.to_string_lossy().into_owned();
    let origin_label = origin.to_string_lossy().into_owned();
    let dest_io = dest.clone();
    let origin_io = origin.clone();
    let title_io = title.clone();
    let artists_io = artists.clone();
    let album_io = album.clone();

    tokio::task::spawn_blocking(move || -> std::result::Result<(), String> {
        if let Some(parent) = dest_io.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
        }
        let ext = dest_io
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_else(|| "part".to_string());
        let temp = dest_io.with_extension(format!("part.{ext}"));
        let _ = std::fs::remove_file(&temp);
        std::fs::copy(&origin_io, &temp).map_err(|e| format!("复制文件失败：{e}"))?;
        if let Err(e) = musyc_library::write_meta_full(
            &temp,
            Some(&title_io),
            &artists_io,
            Some(&album_io),
            lyric.as_deref(),
            cover.as_deref(),
        ) {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("写入标签失败：{e}"));
        }
        if let Err(e) = std::fs::rename(&temp, &dest_io) {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("重命名失败：{e}"));
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("转换任务失败：{e}"))??;

    Ok(TagUpdate {
        title,
        artists,
        album,
        id: dest_label,
        origin: Some(origin_label),
    })
}

fn decode_cover(bytes: &[u8]) -> Option<CoverArt> {
    let image = image::load_from_memory(bytes).ok()?;
    let thumb = image.thumbnail(72, 72);
    let rgb = thumb.to_rgb8();
    let (width, height) = (rgb.width(), rgb.height());
    if width == 0 || height == 0 {
        return None;
    }
    Some(CoverArt {
        width,
        height,
        rgb: rgb.into_raw(),
    })
}

fn local_song(path: &Path, title: Option<String>, duration_ms: u64) -> Song {
    local_song_at(path, path, title, duration_ms)
}

/// `play_path` 是实际播放的文件（优先转换副本），`origin_path` 是扫描到的
/// 原始文件，只用来推断标题 / 歌手，绝不写入。
fn local_song_at(
    play_path: &Path,
    origin_path: &Path,
    title: Option<String>,
    duration_ms: u64,
) -> Song {
    let meta = musyc_library::read_meta(play_path);
    let (file_artist, file_title) = musyc_library::filename_hint(origin_path);
    let converted = play_path != origin_path;
    let album = if converted {
        meta.as_ref()
            .and_then(|m| m.album.clone())
            .unwrap_or_default()
    } else {
        meta.as_ref()
            .map(|m| display_album(m, file_artist.as_deref(), file_title.as_deref()))
            .unwrap_or_default()
    };
    let meta_artists: Vec<String> = meta
        .as_ref()
        .map(|m| {
            m.artists
                .iter()
                .filter(|a| a.as_str() != "未知歌手" && !a.trim().is_empty())
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    // 原始文件名 `歌手 - 标题` 比内嵌标签可靠：多数文件的标签字段存在整组错位。
    // 转换副本是刚写入的正式标签，优先使用标签。
    let artists = if converted {
        meta_artists.clone()
    } else {
        file_artist
            .map(|artist| vec![artist])
            .filter(|artists| !artists.is_empty())
            .unwrap_or(meta_artists)
    };
    let name = if converted {
        meta.as_ref()
            .and_then(|m| m.title.clone())
            .filter(|s| !s.trim().is_empty())
            .or(title)
            .or(file_title)
    } else {
        title.or(file_title).or_else(|| {
            meta.as_ref()
                .and_then(|m| m.title.clone())
                .filter(|s| !s.trim().is_empty())
        })
    }
    .or_else(|| {
        origin_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
    })
    .unwrap_or_else(|| "未知曲目".into());
    let meta_duration = meta.as_ref().map(|m| m.duration_ms).unwrap_or(0);
    Song {
        id: play_path.to_string_lossy().into_owned(),
        name,
        artists,
        album,
        album_id: None,
        duration_ms: if duration_ms > 0 {
            duration_ms
        } else {
            meta_duration
        },
        cover_url: None,
        fee: 0,
        source: SourceKind::Local,
        play_url: None,
        lyric_url: None,
        source_id: None,
        origin: Some(origin_path.to_string_lossy().into_owned()),
    }
}

/// 本地文件经常把歌手、专辑、标题三个字段整组错位。若标签呈现典型轮换
/// （title=歌手 或 album=歌手），把标签里的 artist 字段当作真正的专辑。
fn display_album(
    meta: &musyc_library::EmbeddedMeta,
    hint_artist: Option<&str>,
    hint_title: Option<&str>,
) -> String {
    let album = meta.album.as_deref().map(str::trim).unwrap_or("");
    let title = meta.title.as_deref().map(str::trim).unwrap_or("");
    let (Some(hint_artist), Some(hint_title)) = (
        hint_artist.map(str::trim).filter(|s| !s.is_empty()),
        hint_title.map(str::trim).filter(|s| !s.is_empty()),
    ) else {
        return album.to_string();
    };
    let rotated = (same_text(title, hint_artist) && same_text(album, hint_title))
        || (same_text(title, hint_title) && same_text(album, hint_artist));
    if rotated {
        let artist_album = meta
            .artists
            .iter()
            .filter(|a| a.as_str() != "未知歌手" && !a.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" / ");
        if !artist_album.is_empty() {
            return artist_album;
        }
    }
    album.to_string()
}

fn same_text(left: &str, right: &str) -> bool {
    !left.is_empty() && !right.is_empty() && normalize(left) == normalize(right)
}

fn scan_local_dir(dir: &Path, output_dir: &Path) -> std::result::Result<LocalScan, String> {
    if !dir.exists() {
        return Err(format!("目录不存在：{}", dir.display()));
    }
    if !dir.is_dir() {
        return Err(format!("不是目录：{}", dir.display()));
    }
    let mut files = Vec::new();
    collect_audio_files(dir, output_dir, &mut files);
    files.sort();
    let mut scan = LocalScan::default();
    for path in &files {
        let relative = path.strip_prefix(dir).unwrap_or(path);
        let converted = output_dir.join(relative);
        let play = if converted.exists() {
            converted.as_path()
        } else {
            path.as_path()
        };
        let song = local_song_at(play, path, None, 0);
        let needs_tag = play == path
            || musyc_library::read_meta(play)
                .is_none_or(|m| m.title.as_deref().is_none_or(|t| t.trim().is_empty()));
        if needs_tag {
            // 还没有转换副本的文件，也列入批量补全范围。
            scan.needs_tag.push(song.id.clone());
        }
        scan.songs.push(song);
    }
    Ok(scan)
}

fn collect_audio_files(dir: &Path, output_dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            // 转换输出目录不参与扫描，否则会把副本当成新歌。
            if path.starts_with(output_dir) {
                continue;
            }
            collect_audio_files(&path, output_dir, out);
        } else if file_type.is_file() && input::is_audio_path(&path) && !input::is_m3u_path(&path) {
            out.push(path);
        }
    }
}

/// OpenCC 繁→简实例，进程内只建一次。字典内嵌在二进制里，不碰文件系统。
static OPENCC_T2S: LazyLock<Option<OpenCC>> =
    LazyLock::new(|| OpenCC::from_config(BuiltinConfig::T2s).ok());

/// 统一的比较键：繁→简 + 小写 + 只留字母数字。
/// 繁体文件名和简体接口都归一到同一条路径上再比对。
fn normalize(text: &str) -> String {
    let simplified = OPENCC_T2S
        .as_ref()
        .map(|cc| cc.convert(text))
        .unwrap_or(text.to_string());
    simplified
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn pick_best_match(
    song: &Song,
    hint_artist: Option<&str>,
    hint_title: Option<&str>,
    candidates: &[Song],
) -> Option<Song> {
    let want_title = hint_title
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| song.name.clone());
    let want_artist = hint_artist
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| song.artists.first().cloned())
        .unwrap_or_default();
    let want_title_n = normalize(&want_title);
    let want_artist_n = normalize(&want_artist);
    if want_title_n.is_empty() {
        return None;
    }

    let mut best: Option<(i32, Song)> = None;
    for candidate in candidates {
        let title_n = normalize(&candidate.name);
        let title_score = if title_n == want_title_n {
            100
        } else if !title_n.is_empty()
            && (title_n.contains(&want_title_n) || want_title_n.contains(&title_n))
        {
            60
        } else {
            0
        };
        if title_score == 0 {
            continue;
        }
        let artist_n = normalize(&candidate.artist_text());
        let artist_score = if want_artist_n.is_empty() || artist_n.is_empty() {
            0
        } else if artist_n == want_artist_n {
            40
        } else if artist_n.contains(&want_artist_n) || want_artist_n.contains(&artist_n) {
            25
        } else {
            0
        };
        // 标题不完全一致又对不上歌手，宁可不动文件。
        if !want_artist_n.is_empty() && artist_score == 0 && title_score < 100 {
            continue;
        }
        let mut score = title_score + artist_score;
        if song.duration_ms > 0 && candidate.duration_ms > 0 {
            let diff = song.duration_ms.abs_diff(candidate.duration_ms);
            score += if diff <= 2000 {
                20
            } else if diff <= 5000 {
                10
            } else if diff <= 10000 {
                0
            } else {
                -25
            };
        }
        // 本地文件名能给出歌手时，必须同时有歌手或时长证据，避免只凭
        // 同名标题把另一首歌的标签写进来。
        let threshold = if want_artist_n.is_empty() { 90 } else { 120 };
        if score < threshold {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, candidate.clone()));
        }
    }
    best.map(|(_, song)| song)
}

fn prev_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor.min(text.len())]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn next_boundary(text: &str, cursor: usize) -> usize {
    let cursor = cursor.min(text.len());
    text[cursor..]
        .chars()
        .next()
        .map(|c| cursor + c.len_utf8())
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(name: &str, artist: &str, duration_ms: u64) -> Song {
        Song {
            id: name.into(),
            name: name.into(),
            artists: vec![artist.into()],
            duration_ms,
            ..Song::default()
        }
    }

    #[test]
    fn best_match_prefers_title_and_artist() {
        let local = song("Ground Zero", "April Rain", 372_000);
        let candidates = vec![
            song("Ground Zero", "Some Cover Band", 200_000),
            song("Ground Zero", "April Rain", 372_400),
        ];
        let matched = pick_best_match(&local, Some("April Rain"), Some("Ground Zero"), &candidates)
            .expect("应匹配到原曲");
        assert_eq!(matched.artists, vec!["April Rain"]);
    }

    #[test]
    fn best_match_rejects_unrelated() {
        let local = song("Ground Zero", "April Rain", 372_000);
        let candidates = vec![song("完全无关", "别的歌手", 100_000)];
        assert!(pick_best_match(&local, None, None, &candidates).is_none());
    }

    #[test]
    fn normalize_unifies_traditional_and_simplified() {
        assert_eq!(normalize("世說心語"), normalize("世说心语"));
        assert_eq!(normalize("歷歷萬鄉"), normalize("历历万乡"));
        // 生僻异体（U+8914 褔）OpenCC 字典不收，但网易云官方曲名也写此形，两边一致可直接匹配
        assert_eq!(normalize("大風吹"), normalize("大风吹"));
    }

    #[test]
    fn best_match_matches_traditional_file_name() {
        let local = song("世說心語", "Sun Yanzi", 222_000);
        let candidates = vec![song("世说心语", "孙燕姿", 222_106)];
        let matched = pick_best_match(&local, Some("Sun Yanzi"), Some("世說心語"), &candidates);
        // 艺术家对不上（Sun Yanzi vs 孙燕姿）也至少应凭标题+时长命中
        assert!(matched.is_some());
    }

    #[test]
    fn best_match_needs_artist_or_duration_evidence() {
        let local = song("Ground Zero", "April Rain", 372_000);
        let wrong_artist = vec![song("Ground Zero", "Some Cover Band", 0)];
        assert!(pick_best_match(&local, None, None, &wrong_artist).is_none());
        let same_duration = vec![song("Ground Zero", "Some Cover Band", 372_200)];
        assert!(pick_best_match(&local, None, None, &same_duration).is_some());
    }
}

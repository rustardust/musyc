package dev.musyc.app.ui

import android.app.Application
import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dev.musyc.app.data.Downloader
import dev.musyc.app.data.LocalMusic
import dev.musyc.app.data.Lyric
import dev.musyc.app.data.NeteaseApi
import dev.musyc.app.data.Playlist
import dev.musyc.app.data.RecentStore
import dev.musyc.app.data.Repository
import dev.musyc.app.data.Song
import dev.musyc.app.data.Source
import dev.musyc.app.data.SourceCatalog
import dev.musyc.app.data.SourceDef
import dev.musyc.app.data.SourceLoader
import dev.musyc.app.data.SourceStore
import dev.musyc.app.data.SourceType
import dev.musyc.app.data.Tagger
import dev.musyc.app.play.PlaybackManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

sealed interface Screen {
    data object Home : Screen
    data object Search : Screen
    data object Indie : Screen
    data object Local : Screen
    data object Recent : Screen
    data object Lyrics : Screen
    data class Songs(
        val title: String,
        val subtitle: String,
        val source: SongsSource,
    ) : Screen
}

enum class SongsSource { ONLINE, NETEASE_PLAYLIST, INDIE_VOL, RECENT, LOCAL }

sealed interface SlotKind {
    data class Online(val def: SourceDef) : SlotKind
    data object Indie : SlotKind
    data object Local : SlotKind
    data object Recent : SlotKind
}

data class HomeSlot(
    val kind: SlotKind,
    val title: String,
    val subtitle: String,
    val count: Int = 0,
    val coverUrl: String? = null,
    val loading: Boolean = false,
    val error: String? = null,
) {
    val def: SourceDef? get() = (kind as? SlotKind.Online)?.def
}

data class SongsUiState(
    val title: String = "",
    val subtitle: String = "",
    val songs: List<Song> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
)

data class SearchUiState(
    val query: String = "",
    val loading: Boolean = false,
    val results: List<Song> = emptyList(),
    val playlists: List<Playlist> = emptyList(),
    val error: String? = null,
)

data class IndieUiState(
    val playlists: List<Playlist> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
)

data class TagProgress(val done: Int, val total: Int, val current: String)

class AppViewModel(app: Application) : AndroidViewModel(app) {
    private val recentStore = RecentStore(app)
    private val sourceStore = SourceStore(app)

    private val _stack = MutableStateFlow<List<Screen>>(listOf(Screen.Home))
    val stack = _stack.asStateFlow()

    private val _customSources = MutableStateFlow(sourceStore.custom())
    val customSources = _customSources.asStateFlow()

    private var slotIds: List<String> = sourceStore.slotIds()

    private val _home = MutableStateFlow(buildHome())
    val home = _home.asStateFlow()

    private val _pickerSlot = MutableStateFlow<Int?>(null)
    val pickerSlot = _pickerSlot.asStateFlow()

    private val _songs = MutableStateFlow(SongsUiState())
    val songs = _songs.asStateFlow()

    private val _search = MutableStateFlow(SearchUiState())
    val search = _search.asStateFlow()

    private val _indie = MutableStateFlow(IndieUiState())
    val indie = _indie.asStateFlow()

    private val _playerOpen = MutableStateFlow(false)
    val playerOpen = _playerOpen.asStateFlow()

    private val _lyrics = MutableStateFlow<Lyric?>(null)
    val lyrics = _lyrics.asStateFlow()

    private val _lyricsLoading = MutableStateFlow(false)
    val lyricsLoading = _lyricsLoading.asStateFlow()

    private val _download = MutableStateFlow<Pair<String, Float>?>(null)
    val download = _download.asStateFlow()

    private val _tagging = MutableStateFlow<TagProgress?>(null)
    val tagging = _tagging.asStateFlow()

    private val _messages = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val messages = _messages.asSharedFlow()

    private val _recent = MutableStateFlow<List<Song>>(emptyList())
    val recent = _recent.asStateFlow()

    private var currentDef: SourceDef? = null

    init {
        loadHome()
    }

    private fun buildHome(): List<HomeSlot> {
        val catalog = SourceCatalog.all(_customSources.value)
        val online = slotIds.map { id ->
            val def = catalog.firstOrNull { it.id == id && it.type != SourceType.INDIE }
                ?: SourceCatalog.builtin.first { it.type != SourceType.INDIE }
            HomeSlot(SlotKind.Online(def), def.name, def.desc)
        }
        return online + listOf(
            HomeSlot(SlotKind.Indie, "雀乐推荐", "雀乐 · 每期精选"),
            HomeSlot(SlotKind.Local, "本地音乐", "扫描手机里的音频"),
            HomeSlot(SlotKind.Recent, "最近播放", "本机记录"),
        )
    }

    fun current(): Screen = _stack.value.last()

    fun push(screen: Screen) {
        _stack.update { it + screen }
    }

    fun back(): Boolean {
        val stack = _stack.value
        if (stack.size <= 1) return false
        _stack.value = stack.dropLast(1)
        return true
    }

    fun popToHome() {
        _stack.value = listOf(Screen.Home)
    }

    // ---------- 首页槽位 ----------

    fun loadHome(force: Boolean = false) {
        refreshRecent()
        _home.update { slots ->
            slots.map {
                if (it.kind is SlotKind.Online) it.copy(loading = true, error = null) else it
            }
        }
        _home.value.forEachIndexed { index, slot ->
            when (val kind = slot.kind) {
                is SlotKind.Online -> viewModelScope.launch {
                    runCatching { SourceLoader.load(kind.def) }
                        .onSuccess { songs ->
                            updateSlot(index, songs.size, songs.firstOrNull()?.coverUrl, null)
                        }
                        .onFailure { e -> updateSlot(index, 0, null, friendly(e)) }
                }
                SlotKind.Indie -> viewModelScope.launch {
                    runCatching { Repository.indie(force) }
                        .onSuccess { list ->
                            _indie.value = IndieUiState(playlists = list)
                            updateSlot(index, list.size, list.firstOrNull()?.coverUrl, null)
                        }
                        .onFailure { e -> updateSlot(index, 0, null, friendly(e)) }
                }
                else -> Unit
            }
        }
    }

    private fun updateSlot(index: Int, count: Int, cover: String?, error: String?) {
        _home.update { slots ->
            slots.mapIndexed { i, slot ->
                if (i == index) {
                    slot.copy(loading = false, count = count, coverUrl = cover, error = error)
                } else {
                    slot
                }
            }
        }
    }

    private fun refreshRecent() {
        _recent.value = recentStore.load()
        _home.update { slots ->
            slots.map {
                if (it.kind is SlotKind.Recent) it.copy(count = _recent.value.size) else it
            }
        }
    }

    fun openSlot(slot: HomeSlot) {
        when (val kind = slot.kind) {
            is SlotKind.Online -> openOnline(kind.def)
            SlotKind.Indie -> {
                push(Screen.Indie)
                if (_indie.value.playlists.isEmpty() && !_indie.value.loading) loadIndie()
            }
            SlotKind.Local -> push(Screen.Local)
            SlotKind.Recent -> {
                refreshRecent()
                _songs.value = SongsUiState(
                    title = "最近播放",
                    subtitle = "${_recent.value.size} 首",
                    songs = _recent.value,
                )
                push(Screen.Songs("最近播放", "${_recent.value.size} 首", SongsSource.RECENT))
            }
        }
    }

    fun openOnline(def: SourceDef) {
        currentDef = def
        val already = (_stack.value.lastOrNull() as? Screen.Songs)?.source == SongsSource.ONLINE
        if (!already) {
            _songs.value = SongsUiState(title = def.name, subtitle = def.desc, loading = true)
            push(Screen.Songs(def.name, def.desc, SongsSource.ONLINE))
        } else {
            _songs.update { it.copy(title = def.name, subtitle = def.desc, loading = true, error = null) }
        }
        viewModelScope.launch {
            runCatching { SourceLoader.load(def) }
                .onSuccess { list ->
                    _songs.update { it.copy(songs = list, loading = false) }
                }
                .onFailure { e ->
                    _songs.update { it.copy(loading = false, error = friendly(e)) }
                }
        }
    }

    fun openPicker(index: Int) {
        _pickerSlot.value = index
    }

    fun closePicker() {
        _pickerSlot.value = null
    }

    /** 换绑某个在线槽位并立即刷新。 */
    fun bindSlot(index: Int, def: SourceDef) {
        val ids = slotIds.toMutableList()
        while (ids.size < 3) ids.add(SourceCatalog.builtin[0].id)
        ids[index] = def.id
        slotIds = ids
        sourceStore.saveSlots(ids)
        val fresh = buildHome().toMutableList()
        _home.value.forEachIndexed { i, old ->
            if (i < fresh.size) fresh[i] = fresh[i].copy(count = old.count, coverUrl = old.coverUrl)
        }
        _home.value = fresh
        _pickerSlot.value = null
        loadHome()
    }

    fun importSource(text: String) {
        val def = SourceStore.parseToml(text)
        if (def == null) {
            _messages.tryEmit("TOML 无法识别")
            return
        }
        sourceStore.addCustom(def)
        _customSources.value = sourceStore.custom()
        val fresh = buildHome().toMutableList()
        _home.value.forEachIndexed { i, old ->
            if (i < fresh.size) fresh[i] = fresh[i].copy(count = old.count, coverUrl = old.coverUrl)
        }
        _home.value = fresh
        _messages.tryEmit("已导入 ${def.name}")
    }

    fun removeCustom(id: String) {
        sourceStore.removeCustom(id)
        _customSources.value = sourceStore.custom()
    }

    // ---------- 列表 ----------

    fun loadIndie(force: Boolean = false) {
        viewModelScope.launch {
            _indie.value = _indie.value.copy(loading = true, error = null)
            runCatching { Repository.indie(force) }
                .onSuccess { _indie.value = IndieUiState(playlists = it) }
                .onFailure { e -> _indie.value = IndieUiState(error = friendly(e)) }
        }
    }

    fun openIndiePlaylist(playlist: Playlist) {
        _songs.value = SongsUiState(
            title = playlist.title,
            subtitle = playlist.subtitle,
            songs = playlist.songs,
        )
        push(Screen.Songs(playlist.title, playlist.subtitle, SongsSource.INDIE_VOL))
    }

    suspend fun scanLocal(context: Context): List<Song> = withContext(Dispatchers.IO) {
        LocalMusic.scan(context)
    }

    fun showLocal(songs: List<Song>) {
        _songs.value = SongsUiState(title = "本地音乐", subtitle = "${songs.size} 首", songs = songs)
        push(Screen.Songs("本地音乐", "${songs.size} 首", SongsSource.LOCAL))
    }

    fun deleteRecent(song: Song) {
        recentStore.remove(song)
        refreshRecent()
        _songs.update { st ->
            val list = st.songs.filterNot { s -> s.key == song.key }
            st.copy(songs = list, subtitle = "${list.size} 首")
        }
        _messages.tryEmit("已移除")
    }

    fun refreshCurrent(context: Context) {
        when (val screen = current()) {
            is Screen.Songs -> when (screen.source) {
                SongsSource.ONLINE -> currentDef?.let { openOnline(it) }
                SongsSource.LOCAL -> viewModelScope.launch {
                    val list = runCatching { scanLocal(context) }.getOrDefault(emptyList())
                    _songs.update { it.copy(songs = list, subtitle = "${list.size} 首") }
                    _messages.tryEmit("已刷新")
                }
                SongsSource.RECENT -> {
                    refreshRecent()
                    _songs.update { it.copy(songs = _recent.value, subtitle = "${_recent.value.size} 首") }
                    _messages.tryEmit("已刷新")
                }
                SongsSource.INDIE_VOL -> {
                    val title = _songs.value.title
                    _indie.value.playlists.firstOrNull { it.title == title }?.let { p ->
                        _songs.update { it.copy(songs = p.songs) }
                        _messages.tryEmit("已刷新")
                    }
                }
                SongsSource.NETEASE_PLAYLIST -> Unit
            }
            Screen.Indie -> loadIndie(force = true)
            Screen.Home -> loadHome(force = true)
            Screen.Local -> Unit
            else -> Unit
        }
    }

    // ---------- 搜索 ----------

    fun search(query: String) {
        _search.value = _search.value.copy(query = query)
        if (query.isBlank()) {
            _search.value = SearchUiState()
            return
        }
        viewModelScope.launch {
            _search.value = _search.value.copy(loading = true, error = null)
            val songs = async { runCatching { NeteaseApi.searchSongs(query) } }
            val playlists = async { runCatching { NeteaseApi.searchPlaylists(query) } }
            val songResult = songs.await()
            val listResult = playlists.await()
            val error = songResult.exceptionOrNull() ?: listResult.exceptionOrNull()
            _search.value = SearchUiState(
                query = query,
                results = songResult.getOrDefault(emptyList()),
                playlists = listResult.getOrDefault(emptyList()),
                error = error?.let(::friendly),
            )
        }
    }

    /** 打开搜索到的网易云歌单，拉取全部曲目后进入歌曲页。 */
    fun openPlaylist(playlist: Playlist) {
        _songs.value = SongsUiState(title = playlist.title, subtitle = playlist.subtitle, loading = true)
        push(Screen.Songs(playlist.title, playlist.subtitle, SongsSource.NETEASE_PLAYLIST))
        viewModelScope.launch {
            runCatching { NeteaseApi.playlistSongs(playlist.id) }
                .onSuccess { _songs.update { s -> s.copy(songs = it, loading = false) } }
                .onFailure { e -> _songs.update { s -> s.copy(loading = false, error = friendly(e)) } }
        }
    }

    // ---------- 打开链接 / 本地文件 / M3U ----------

    fun openLink(text: String) {
        val raw = text.trim()
        if (raw.isEmpty()) return
        val id = Regex("[?&]id=(\\d+)").find(raw)?.groupValues?.get(1)
            ?: Regex("/song/(\\d+)").find(raw)?.groupValues?.get(1)
            ?: Regex("/playlist/(\\d+)").find(raw)?.groupValues?.get(1)
            ?: raw.takeIf { it.matches(Regex("\\d+")) }
        if (id == null) {
            _messages.tryEmit("无法识别链接")
            return
        }
        val playlist = raw.contains("playlist") || raw.contains("歌单")
        viewModelScope.launch {
            if (playlist) {
                _songs.value = SongsUiState(title = "网易歌单", subtitle = id, loading = true)
                push(Screen.Songs("网易歌单", id, SongsSource.NETEASE_PLAYLIST))
                runCatching { NeteaseApi.playlistSongs(id) }
                    .onSuccess { _songs.update { s -> s.copy(songs = it, loading = false, subtitle = "${it.size} 首") } }
                    .onFailure { e -> _songs.update { s -> s.copy(loading = false, error = friendly(e)) } }
            } else {
                val list = runCatching { NeteaseApi.songDetail(listOf(id)) }.getOrDefault(emptyList())
                if (list.isEmpty()) {
                    _messages.tryEmit("未找到歌曲")
                } else {
                    play(list, 0)
                    _messages.tryEmit("已打开")
                }
            }
        }
    }

    /** 直接播放一条可访问的音频地址（M3U / 打开链接里的 http 条目）。 */
    fun playRemote(url: String) {
        val name = url.substringAfterLast('/').substringBefore('?').substringBeforeLast('.')
        val song = Song(
            id = url,
            title = name.ifBlank { "网络音频" },
            source = Source.CUSTOM,
            playUrl = url,
        )
        _songs.value = SongsUiState(title = "网络音频", subtitle = "1 首", songs = listOf(song))
        push(Screen.Songs("网络音频", "1 首", SongsSource.ONLINE))
        play(listOf(song), 0)
    }

    fun openLocalUri(context: Context, uri: Uri) {
        val name = displayName(context, uri) ?: "本地音频"
        val title = name.substringBeforeLast('.')
        val song = Song(
            id = uri.toString(),
            title = title.ifBlank { name },
            source = Source.LOCAL,
            localUri = uri.toString(),
        )
        _songs.value = SongsUiState(title = "本地文件", subtitle = "1 首", songs = listOf(song))
        push(Screen.Songs("本地文件", "1 首", SongsSource.LOCAL))
        play(listOf(song), 0)
    }

    fun openM3u(context: Context, uri: Uri) {
        viewModelScope.launch {
            val text = runCatching {
                withContext(Dispatchers.IO) {
                    context.contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() }
                }
            }.getOrNull() ?: run {
                _messages.tryEmit("读取歌单失败")
                return@launch
            }
            val entries = text.lineSequence()
                .map(String::trim)
                .filter { it.isNotEmpty() && !it.startsWith("#") }
                .toList()
            val songs = entries.mapIndexedNotNull { i, entry ->
                when {
                    entry.startsWith("http") -> Song(
                        id = "m3u-$i",
                        title = entry.substringAfterLast('/')
                            .substringBeforeLast('.')
                            .ifBlank { "网络音频" },
                        source = Source.CUSTOM,
                        playUrl = entry,
                    )
                    entry.startsWith("file://") || entry.startsWith("/") -> null
                    else -> null
                }
            }
            if (songs.isEmpty()) {
                _messages.tryEmit("没有可播条目")
                return@launch
            }
            _songs.value = SongsUiState(title = "M3U", subtitle = "${songs.size} 首", songs = songs)
            push(Screen.Songs("M3U", "${songs.size} 首", SongsSource.ONLINE))
            play(songs, 0)
            _messages.tryEmit("已打开 ${songs.size} 首")
        }
    }

    private fun displayName(context: Context, uri: Uri): String? =
        runCatching {
            context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { c -> if (c.moveToFirst()) c.getString(0) else null }
        }.getOrNull()

    // ---------- 标签补全 ----------

    /** 补全一首或一批本地歌曲：匹配网易元数据后生成带标签/封面/歌词的副本。 */
    fun tagSongs(context: Context, list: List<Song>) {
        if (_tagging.value != null) {
            _messages.tryEmit("正在补全中")
            return
        }
        val pending = list.filter { Tagger.needsTag(it) }
        if (pending.isEmpty()) {
            _messages.tryEmit("标签已完整")
            return
        }
        viewModelScope.launch {
            var ok = 0
            var fail = 0
            _tagging.value = TagProgress(0, pending.size, "")
            pending.forEachIndexed { i, song ->
                _tagging.value = TagProgress(i, pending.size, song.title)
                runCatching { Tagger.complete(context, song) }
                    .onSuccess { ok++ }
                    .onFailure { fail++ }
            }
            _tagging.value = null
            _messages.tryEmit("补全完成：成功 $ok 失败 $fail")
            if ((current() as? Screen.Songs)?.source == SongsSource.LOCAL) {
                val list = runCatching { scanLocal(context) }.getOrNull() ?: return@launch
                _songs.update { it.copy(songs = list, subtitle = "${list.size} 首") }
            }
        }
    }

    // ---------- 播放 ----------

    fun play(list: List<Song>, index: Int) {
        PlaybackManager.play(list, index)
        refreshRecentSoon()
    }

    fun playSingle(song: Song, list: List<Song> = listOf(song)) {
        val index = list.indexOfFirst { it.key == song.key }.coerceAtLeast(0)
        play(list, index)
    }

    private fun refreshRecentSoon() {
        viewModelScope.launch {
            delay(1500)
            PlaybackManager.state.value.song?.let { recentStore.push(it) }
            refreshRecent()
        }
    }

    fun openPlayer() {
        _playerOpen.value = true
    }

    fun closePlayer() {
        _playerOpen.value = false
    }

    private var lyricsJob: kotlinx.coroutines.Job? = null

    /** 打开全页歌词；重复点击同一页时仅刷新，不再叠栈。 */
    fun openLyrics() {
        val song = PlaybackManager.state.value.song ?: return
        _playerOpen.value = false
        if (_stack.value.lastOrNull() !is Screen.Lyrics) push(Screen.Lyrics)
        _lyrics.value = null
        _lyricsLoading.value = true
        lyricsJob?.cancel()
        lyricsJob = viewModelScope.launch {
            _lyrics.value = runCatching { Repository.lyric(song) }.getOrNull()
            _lyricsLoading.value = false
        }
    }

    fun closeLyrics() {
        if (_stack.value.lastOrNull() is Screen.Lyrics) back()
    }

    fun downloadSong(context: Context) {
        val song = PlaybackManager.state.value.song ?: return
        if (_download.value != null) {
            _messages.tryEmit("已有下载")
            return
        }
        viewModelScope.launch {
            _download.value = song.title to 0f
            try {
                val resolved = Repository.resolve(song) ?: error("无法解析地址")
                val path = Downloader.download(context, resolved) { p ->
                    _download.update { song.title to p }
                }
                _messages.tryEmit("已保存 $path")
            } catch (e: Exception) {
                _messages.tryEmit(friendly(e))
            } finally {
                _download.value = null
            }
        }
    }

    fun toast(text: String) {
        _messages.tryEmit(text)
    }

    private fun friendly(error: Throwable): String =
        error.message?.take(80)?.ifBlank { null } ?: "网络错误"
}

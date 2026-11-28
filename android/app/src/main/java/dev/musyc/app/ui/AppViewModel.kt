package dev.musyc.app.ui

import android.app.Application
import android.content.Context
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dev.musyc.app.data.LocalMusic
import dev.musyc.app.data.Lyric
import dev.musyc.app.data.Playlist
import dev.musyc.app.data.RecentStore
import dev.musyc.app.data.Repository
import dev.musyc.app.data.Song
import dev.musyc.app.play.PlaybackManager
import dev.musyc.app.data.Downloader
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
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

enum class SongsSource { NETEASE, NETEASE_PLAYLIST, STREETVOICE, INDIE_VOL, RECENT, LOCAL }

data class HomeSlot(
    val source: SongsSource,
    val title: String,
    val subtitle: String,
    val count: Int = 0,
    val coverUrl: String? = null,
    val loading: Boolean = false,
    val error: String? = null,
)

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

class AppViewModel(app: Application) : AndroidViewModel(app) {
    private val recentStore = RecentStore(app)

    private val _stack = MutableStateFlow<List<Screen>>(listOf(Screen.Home))
    val stack = _stack.asStateFlow()

    private val _home = MutableStateFlow(
        listOf(
            HomeSlot(SongsSource.NETEASE, "网易原创", "网易原创榜 · 官方直链"),
            HomeSlot(SongsSource.STREETVOICE, "街声推荐", "街声 · HLS"),
            HomeSlot(SongsSource.INDIE_VOL, "雀乐推荐", "雀乐 · 每期精选"),
            HomeSlot(SongsSource.LOCAL, "本地音乐", "扫描手机里的音频"),
            HomeSlot(SongsSource.RECENT, "最近播放", "本机记录"),
        )
    )
    val home = _home.asStateFlow()

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

    private val _messages = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val messages = _messages.asSharedFlow()

    private val _recent = MutableStateFlow<List<Song>>(emptyList())
    val recent = _recent.asStateFlow()

    init {
        loadHome()
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

    fun loadHome(force: Boolean = false) {
        refreshRecent()
        viewModelScope.launch {
            _home.update { slots -> slots.map { if (it.source == SongsSource.NETEASE) it.copy(loading = true, error = null) else it } }
            launch {
                runCatching { Repository.neteaseChart(force) }
                    .onSuccess { p -> updateSlot(SongsSource.NETEASE, p.songs.size, p.coverUrl, null) }
                    .onFailure { e -> updateSlot(SongsSource.NETEASE, 0, null, friendly(e)) }
            }
            launch {
                _home.update { slots -> slots.map { if (it.source == SongsSource.STREETVOICE) it.copy(loading = true, error = null) else it } }
                runCatching { Repository.streetVoice(force) }
                    .onSuccess { p -> updateSlot(SongsSource.STREETVOICE, p.songs.size, p.coverUrl, null) }
                    .onFailure { e -> updateSlot(SongsSource.STREETVOICE, 0, null, friendly(e)) }
            }
            launch {
                _home.update { slots -> slots.map { if (it.source == SongsSource.INDIE_VOL) it.copy(loading = true, error = null) else it } }
                runCatching { Repository.indie(force) }
                    .onSuccess { list ->
                        _indie.value = IndieUiState(playlists = list)
                        updateSlot(SongsSource.INDIE_VOL, list.size, list.firstOrNull()?.coverUrl, null)
                    }
                    .onFailure { e -> updateSlot(SongsSource.INDIE_VOL, 0, null, friendly(e)) }
            }
        }
    }

    private fun updateSlot(source: SongsSource, count: Int, cover: String?, error: String?) {
        _home.update { slots ->
            slots.map {
                if (it.source == source) it.copy(loading = false, count = count, coverUrl = cover, error = error)
                else it
            }
        }
    }

    private fun refreshRecent() {
        _recent.value = recentStore.load()
        _home.update { slots ->
            slots.map {
                if (it.source == SongsSource.RECENT) it.copy(count = _recent.value.size) else it
            }
        }
    }

    fun openSlot(slot: HomeSlot) {
        when (slot.source) {
            SongsSource.NETEASE -> viewModelScope.launch {
                _songs.value = SongsUiState(title = "网易原创", subtitle = "官方直链", loading = true)
                push(Screen.Songs("网易原创", "官方直链", SongsSource.NETEASE))
                runCatching { Repository.neteaseChart() }
                    .onSuccess { _songs.update { s -> s.copy(songs = it.songs, loading = false) } }
                    .onFailure { e -> _songs.update { s -> s.copy(loading = false, error = friendly(e)) } }
            }
            SongsSource.STREETVOICE -> viewModelScope.launch {
                _songs.value = SongsUiState(title = "街声推荐", subtitle = "街声", loading = true)
                push(Screen.Songs("街声推荐", "街声", SongsSource.STREETVOICE))
                runCatching { Repository.streetVoice() }
                    .onSuccess { _songs.update { s -> s.copy(songs = it.songs, loading = false) } }
                    .onFailure { e -> _songs.update { s -> s.copy(loading = false, error = friendly(e)) } }
            }
            SongsSource.NETEASE_PLAYLIST -> Unit
            SongsSource.INDIE_VOL -> {
                push(Screen.Indie)
                if (_indie.value.playlists.isEmpty() && !_indie.value.loading) loadIndie()
            }
            SongsSource.LOCAL -> push(Screen.Local)
            SongsSource.RECENT -> {
                refreshRecent()
                _songs.value = SongsUiState(title = "最近播放", subtitle = "${_recent.value.size} 首", songs = _recent.value)
                push(Screen.Songs("最近播放", "${_recent.value.size} 首", SongsSource.RECENT))
            }
        }
    }

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

    fun search(query: String) {
        _search.value = _search.value.copy(query = query)
        if (query.isBlank()) {
            _search.value = SearchUiState()
            return
        }
        viewModelScope.launch {
            _search.value = _search.value.copy(loading = true, error = null)
            val songs = async { runCatching { dev.musyc.app.data.NeteaseApi.searchSongs(query) } }
            val playlists = async { runCatching { dev.musyc.app.data.NeteaseApi.searchPlaylists(query) } }
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
            runCatching { dev.musyc.app.data.NeteaseApi.playlistSongs(playlist.id) }
                .onSuccess { _songs.update { s -> s.copy(songs = it, loading = false) } }
                .onFailure { e -> _songs.update { s -> s.copy(loading = false, error = friendly(e)) } }
        }
    }

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
            kotlinx.coroutines.delay(1500)
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

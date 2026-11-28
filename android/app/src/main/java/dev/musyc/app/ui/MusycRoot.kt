package dev.musyc.app.ui

import android.Manifest
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import dev.musyc.app.play.PlaybackManager
import kotlinx.coroutines.launch

@Composable
fun MusycRoot(vm: AppViewModel) {
    val stack by vm.stack.collectAsState()
    val home by vm.home.collectAsState()
    val songs by vm.songs.collectAsState()
    val search by vm.search.collectAsState()
    val indie by vm.indie.collectAsState()
    val player by PlaybackManager.state.collectAsState()
    val playerOpen by vm.playerOpen.collectAsState()
    val lyrics by vm.lyrics.collectAsState()
    val lyricsOpen by vm.lyricsOpen.collectAsState()
    val lyricsLoading by vm.lyricsLoading.collectAsState()
    val download by vm.download.collectAsState()

    val snackbar = remember { SnackbarHostState() }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var scanning by remember { mutableStateOf(false) }
    var sleepOpen by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        vm.messages.collect { snackbar.showSnackbar(it) }
    }
    LaunchedEffect(player.message) {
        player.message?.let {
            snackbar.showSnackbar(it)
            PlaybackManager.consumeMessage()
        }
    }

    val current = stack.lastOrNull() ?: Screen.Home
    fun startScan() {
        scope.launch {
            scanning = true
            runCatching { vm.scanLocal(context) }
                .onSuccess { vm.showLocal(it) }
                .onFailure { vm.toast("扫描失败") }
            scanning = false
        }
    }
    val permission = if (Build.VERSION.SDK_INT >= 33) {
        Manifest.permission.READ_MEDIA_AUDIO
    } else {
        Manifest.permission.READ_EXTERNAL_STORAGE
    }
    val requestPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (granted) startScan() else vm.toast("需要音频权限")
    }

    BackHandler(enabled = stack.size > 1 && !playerOpen) {
        vm.back()
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        bottomBar = {
            MiniPlayer(
                state = player,
                onOpen = vm::openPlayer,
                onToggle = { PlaybackManager.toggle() },
                onNext = { PlaybackManager.next() },
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            when (val screen = current) {
                Screen.Home -> HomeScreen(
                    slots = home,
                    onRefresh = { vm.loadHome(force = true) },
                    onSearch = { vm.push(Screen.Search) },
                    onOpen = vm::openSlot,
                )
                Screen.Search -> SearchScreen(
                    state = search,
                    playingKey = player.song?.key,
                    onBack = { vm.back() },
                    onQuery = vm::search,
                    onPlay = { index -> vm.play(search.results, index) },
                )
                Screen.Indie -> IndieScreen(
                    state = indie,
                    onBack = { vm.back() },
                    onOpen = { index -> indie.playlists.getOrNull(index)?.let(vm::openIndiePlaylist) },
                )
                Screen.Local -> LocalScreen(
                    scanning = scanning,
                    onBack = { vm.back() },
                    onScan = { requestPermission.launch(permission) },
                )
                is Screen.Songs -> SongsScreen(
                    state = songs,
                    playingKey = player.song?.key,
                    onBack = { vm.back() },
                    onPlay = { index -> vm.play(songs.songs, index) },
                )
                Screen.Recent -> SongsScreen(
                    state = songs,
                    playingKey = player.song?.key,
                    onBack = { vm.back() },
                    onPlay = { index -> vm.play(songs.songs, index) },
                )
            }
        }
    }

    if (playerOpen) {
        PlayerSheet(
            state = player,
            download = download,
            onDismiss = vm::closePlayer,
            onToggle = { PlaybackManager.toggle() },
            onNext = { PlaybackManager.next() },
            onPrevious = { PlaybackManager.previous() },
            onSeek = { PlaybackManager.seekTo(it) },
            onLyrics = vm::openLyrics,
            onDownload = { vm.downloadSong(context) },
            onSleep = { sleepOpen = true },
        )
    }
    if (lyricsOpen) {
        LyricsSheet(
            lyric = lyrics,
            loading = lyricsLoading,
            positionMs = player.positionMs,
            onDismiss = vm::closeLyrics,
        )
    }
    if (sleepOpen) {
        SleepDialog(
            onDismiss = { sleepOpen = false },
            onPick = { minutes ->
                PlaybackManager.setSleep(minutes)
                sleepOpen = false
            },
        )
    }
}

package dev.musyc.app.ui

import android.Manifest
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.unit.dp
import dev.musyc.app.data.SourceCatalog
import dev.musyc.app.play.PlaybackManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MusycRoot(vm: AppViewModel) {
    val stack by vm.stack.collectAsState()
    val home by vm.home.collectAsState()
    val songs by vm.songs.collectAsState()
    val search by vm.search.collectAsState()
    val indie by vm.indie.collectAsState()
    val player by PlaybackManager.state.collectAsState()
    val lyrics by vm.lyrics.collectAsState()
    val lyricsLoading by vm.lyricsLoading.collectAsState()
    val download by vm.download.collectAsState()
    val tagging by vm.tagging.collectAsState()
    val pickerSlot by vm.pickerSlot.collectAsState()
    val customSources by vm.customSources.collectAsState()

    val snackbar = remember { SnackbarHostState() }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var scanning by remember { mutableStateOf(false) }
    var sleepOpen by remember { mutableStateOf(false) }
    var queueOpen by remember { mutableStateOf(false) }
    var linkOpen by remember { mutableStateOf(false) }
    var linkText by remember { mutableStateOf("") }

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

    val importToml = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri != null) {
            scope.launch {
                val text = withContext(Dispatchers.IO) {
                    runCatching {
                        context.contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() }
                    }.getOrNull()
                }
                if (text == null) vm.toast("读取失败") else vm.importSource(text)
            }
        }
    }
    val openAudio = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        uri?.let {
            runCatching {
                context.contentResolver.takePersistableUriPermission(
                    it,
                    android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION,
                )
            }
            vm.openLocalUri(context, it)
        }
    }
    val openM3u = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri -> uri?.let { vm.openM3u(context, it) } }

    BackHandler(enabled = stack.size > 1) {
        vm.back()
    }

    Scaffold(
        // 顶部安全区由各页 TopAppBar 自行处理，这里只保留底部，避免顶部 padding 叠加。
        contentWindowInsets = WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom),
        snackbarHost = { SnackbarHost(snackbar) },
        bottomBar = {
            MiniPlayer(
                state = player,
                onOpen = vm::toggleLyrics,
                onMode = { PlaybackManager.cycleMode() },
                onToggle = { PlaybackManager.toggle() },
                onNext = { PlaybackManager.next() },
                onQueue = { queueOpen = true },
                onDownload = { vm.downloadSong(context) },
                onSleep = { sleepOpen = true },
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            when (val screen = current) {
                Screen.Home -> HomeScreen(
                    slots = home,
                    onRefresh = { vm.loadHome(force = true) },
                    onSearch = { vm.push(Screen.Search) },
                    onOpenLink = { linkOpen = true },
                    onSwap = vm::openPicker,
                    onOpen = vm::openSlot,
                )
                Screen.Search -> SearchScreen(
                    state = search,
                    playingKey = player.song?.key,
                    onBack = { vm.back() },
                    onQuery = vm::search,
                    onPlay = { index -> vm.play(search.results, index) },
                    onOpenPlaylist = { index -> search.playlists.getOrNull(index)?.let(vm::openPlaylist) },
                )
                Screen.Indie -> IndieScreen(
                    state = indie,
                    onBack = { vm.back() },
                    onOpen = { index -> indie.playlists.getOrNull(index)?.let(vm::openIndiePlaylist) },
                    onRefresh = { vm.loadIndie(force = true) },
                )
                Screen.Local -> LocalScreen(
                    scanning = scanning,
                    onBack = { vm.back() },
                    onScan = { requestPermission.launch(permission) },
                    onOpenFile = { openAudio.launch(arrayOf("audio/*")) },
                    onOpenM3u = { openM3u.launch(arrayOf("*/*")) },
                )
                is Screen.Songs -> {
                    val local = screen.source == SongsSource.LOCAL
                    val recent = screen.source == SongsSource.RECENT
                    SongsScreen(
                        state = songs,
                        playingKey = player.song?.key,
                        canTag = local,
                        tagging = if (local) tagging else null,
                        onBack = { vm.back() },
                        onPlay = { index -> vm.play(songs.songs, index) },
                        onRefresh = { vm.refreshCurrent(context) },
                        onTagAll = { vm.tagSongs(context, songs.songs) },
                        onTagOne = if (local) { song -> vm.tagSongs(context, listOf(song)) } else null,
                        onDelete = if (recent) vm::deleteRecent else null,
                    )
                }
                Screen.Lyrics -> LyricsScreen(
                    lyric = lyrics,
                    loading = lyricsLoading,
                    positionMs = player.positionMs,
                )
                Screen.Recent -> SongsScreen(
                    state = songs,
                    playingKey = player.song?.key,
                    onBack = { vm.back() },
                    onPlay = { index -> vm.play(songs.songs, index) },
                    onRefresh = { vm.refreshCurrent(context) },
                    onDelete = vm::deleteRecent,
                )
            }
        }
    }

    if (queueOpen) {
        QueueSheet(
            queue = player.queue,
            index = player.index,
            onDismiss = { queueOpen = false },
            onPick = { index ->
                PlaybackManager.play(player.queue, index)
                queueOpen = false
            },
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
    if (linkOpen) {
        AlertDialog(
            onDismissRequest = { linkOpen = false },
            title = { Text("打开链接 / ID") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        "支持歌曲 ID、歌单 ID、网易云链接或 http 音频地址",
                        style = androidx.compose.material3.MaterialTheme.typography.bodySmall,
                    )
                    OutlinedTextField(
                        value = linkText,
                        onValueChange = { linkText = it },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    val text = linkText.trim()
                    linkOpen = false
                    if (text.startsWith("http") && !text.contains("music.163.com")) {
                        scope.launch { vm.playRemote(text) }
                    } else if (text.isNotEmpty()) {
                        vm.openLink(text)
                    }
                    linkText = ""
                }) { Text("打开") }
            },
            dismissButton = {
                TextButton(onClick = { linkOpen = false }) { Text("取消") }
            },
        )
    }
    pickerSlot?.let { index ->
        SourcePickerDialog(
            sources = SourceCatalog.all(customSources).filter { it.type != dev.musyc.app.data.SourceType.INDIE },
            currentId = home.getOrNull(index)?.def?.id,
            onPick = { def -> vm.bindSlot(index, def) },
            onImport = {
                vm.closePicker()
                importToml.launch(arrayOf("text/*", "application/octet-stream", "*/*"))
            },
            onRemove = { def -> vm.removeCustom(def.id) },
            onDismiss = vm::closePicker,
        )
    }
}

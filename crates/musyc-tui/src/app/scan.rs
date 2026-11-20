use super::*;
pub(super) fn local_origin(song: &Song) -> PathBuf {
    song.origin
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&song.id))
}

pub(super) fn same_dir(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

pub(super) fn decode_cover(bytes: &[u8]) -> Option<CoverArt> {
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

pub(super) fn local_song(path: &Path, title: Option<String>, duration_ms: u64) -> Song {
    local_song_at(path, path, title, duration_ms)
}

/// `play_path` 是实际播放的文件（优先转换副本），`origin_path` 是扫描到的
/// 原始文件，只用来推断标题 / 歌手，绝不写入。
pub(super) fn local_song_at(
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
pub(super) fn display_album(
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

pub(super) fn same_text(left: &str, right: &str) -> bool {
    !left.is_empty() && !right.is_empty() && normalize(left) == normalize(right)
}

pub(super) fn scan_local_dir(
    dir: &Path,
    output_dir: &Path,
) -> std::result::Result<LocalScan, String> {
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

pub(super) fn collect_audio_files(dir: &Path, output_dir: &Path, out: &mut Vec<PathBuf>) {
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

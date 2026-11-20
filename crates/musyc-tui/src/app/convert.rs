use super::*;
/// 复制原始文件到转换目录，写入标签 / 歌词 / 封面，再原子改名。
/// 原始文件始终保持不动。
pub(super) async fn convert_and_tag(
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

pub(super) async fn prepare_fallback(
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
pub(super) struct ConvertCtx<'a> {
    pub(super) source_dir: &'a Path,
    pub(super) output_dir: &'a Path,
}

/// 补全流程用到的三个客户端：网易云 API、网易云专用 HTTP、外部库（直连、无代理）。
pub(super) struct ConvertClients<'a> {
    pub(super) api: &'a ApiClient,
    pub(super) http: &'a reqwest::Client,
    pub(super) web: &'a reqwest::Client,
}

/// 单首 / 批量共用的补全流程：网易云搜索 → pick_best_match → convert_and_tag；
/// 未命中或转换失败时走 MusicBrainz/LRCLIB 外部回退。
/// 全部失败只返回 Err(原因)，由调用方决定 log_failure 与计数。
pub(super) async fn resolve_and_convert(
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
pub(super) async fn write_converted(
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

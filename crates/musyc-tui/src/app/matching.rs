use super::*;
/// OpenCC 繁→简实例，进程内只建一次。字典内嵌在二进制里，不碰文件系统。
pub(super) static OPENCC_T2S: LazyLock<Option<OpenCC>> =
    LazyLock::new(|| OpenCC::from_config(BuiltinConfig::T2s).ok());

/// 统一的比较键：繁→简 + 小写 + 只留字母数字。
/// 繁体文件名和简体接口都归一到同一条路径上再比对。
pub(super) fn normalize(text: &str) -> String {
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

pub(super) fn pick_best_match(
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

pub(super) fn prev_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor.min(text.len())]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

pub(super) fn next_boundary(text: &str, cursor: usize) -> usize {
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

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, HighlightSpacing, Paragraph, Row, Table, TableState, Wrap};

use musyc_core::model::{Song, SourceKind};
use musyc_core::util::{format_duration, truncate_width};

use crate::theme::Theme;

pub(super) fn empty_state_message(loading: bool, empty: &str) -> &str {
    if loading { "加载中…" } else { empty }
}

pub(super) fn draw_message(frame: &mut Frame, area: Rect, message: &str, style: Style) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let rect = Rect {
        y: area.y + area.height / 2,
        height: 1,
        ..area
    };
    let line = Line::from(Span::styled(message.to_string(), style)).alignment(Alignment::Center);
    frame.render_widget(Paragraph::new(line), rect);
}

/// 列表为空时的错误提示，允许换行并保留重试入口。
pub(super) fn draw_error(frame: &mut Frame, area: Rect, message: &str, _theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let height = area.height.clamp(1, 5);
    let rect = Rect {
        y: area.y + area.height.saturating_sub(height) / 2,
        height,
        ..area
    };
    let text = format!("{message}\n按 R 重试");
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        rect,
    );
}

// 首页 / 列表标题行：`‹ 标题`，仅加粗，不使用颜色。
pub(super) fn heading_line(title: &str, _theme: &Theme) -> Line<'static> {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    Line::from(vec![
        Span::styled("‹ ", bold),
        Span::styled(title.to_string(), bold),
    ])
}

/// 拆出列表区：第一行是 `‹ 标题`，剩余给表格 / 提示。
pub(super) fn list_split(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
    (chunks[0], chunks[1])
}

pub(super) fn song_table<'a>(
    songs: &'a [Song],
    current_key: Option<&str>,
    theme: &Theme,
    width: u16,
    highlight_current: bool,
) -> Table<'a> {
    let spacing = 1usize;
    // 选中行符号（highlight_symbol("  ") + HighlightSpacing::Always）占两列，
    // 但 Constraint 没有为它留位置，所有列都要按这个偏移收缩，否则右侧时长列会被挤出可视区。
    let symbol_w = 2usize;
    // 序号列按实际条目数计算位数，1000+ 首时不会截断。
    let idx_w = songs.len().to_string().len().max(2);
    let dur_w = songs
        .iter()
        .map(|song| {
            if song.duration_ms > 0 {
                format_duration(song.duration_ms).len()
            } else {
                5
            }
        })
        .max()
        .unwrap_or(5)
        .max(5);
    // 街声没有专辑信息，固定只显示 # / 标题 / 歌手 / 时长；
    // 其他来源在宽度足够（约 96 列以上）时才加入专辑列。
    let has_album = songs.iter().any(|song| !song.album.trim().is_empty())
        && !songs
            .iter()
            .all(|song| song.source == SourceKind::StreetVoice);
    let total = width as usize;
    let base = symbol_w + idx_w + dur_w;
    let mut show_album = false;
    let mut title_w = 0usize;
    let mut artist_w = 0usize;
    let mut album_w = 0usize;
    if has_album {
        let content = total.saturating_sub(base + 4 * spacing);
        let title = (content * 40 / 100).max(28);
        let artist = (content * 22 / 100).max(18);
        let album = content.saturating_sub(title + artist);
        if album >= 31 {
            show_album = true;
            title_w = title;
            artist_w = artist;
            album_w = album;
        }
    }
    if !show_album {
        let content = total.saturating_sub(base + 3 * spacing);
        title_w = ((content * 58 / 100).max(10)).min(content);
        artist_w = content.saturating_sub(title_w);
    }

    let rows = songs.iter().enumerate().map(|(i, song)| {
        let is_current = current_key.is_some_and(|key| key == song.key());
        let title = truncate_width(&song.name, title_w);
        let artist = truncate_width(&song.artist_text(), artist_w);
        let duration = if song.duration_ms > 0 {
            format_duration(song.duration_ms)
        } else {
            "--:--".to_string()
        };
        let mut cells = vec![
            Cell::from(format!("{:>idx_w$}", i + 1)),
            Cell::from(title),
            Cell::from(artist),
        ];
        if show_album {
            let album = if song.album.trim().is_empty() {
                "—".to_string()
            } else {
                truncate_width(&song.album, album_w)
            };
            cells.push(Cell::from(album));
        }
        cells.push(Cell::from(format!("{:>dur_w$}", duration)));
        let row = Row::new(cells).height(1);
        if is_current {
            row.style(Style::default().fg(theme.bender))
        } else {
            row
        }
    });

    let mut widths = vec![
        Constraint::Length(idx_w as u16),
        Constraint::Length(title_w as u16),
        Constraint::Length(artist_w as u16),
    ];
    if show_album {
        widths.push(Constraint::Length(album_w as u16));
    }
    widths.push(Constraint::Length(dur_w as u16));

    // 选中行若恰好是当前播放歌曲，则与 bender 保持一致（红黄中间值）。
    let highlight_fg = if highlight_current {
        theme.bender
    } else {
        theme.accent
    };

    Table::new(rows, widths)
        .column_spacing(spacing as u16)
        .highlight_symbol("  ")
        .highlight_spacing(HighlightSpacing::Always)
        .row_highlight_style(
            Style::default()
                .fg(highlight_fg)
                .add_modifier(Modifier::BOLD),
        )
}

#[allow(clippy::too_many_arguments)]
pub fn render_songs(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    songs: &[Song],
    current_key: Option<&str>,
    state: &mut TableState,
    title: &str,
    loading: bool,
    error: Option<&str>,
    empty: &str,
) {
    // 有数据时错误只占标题，列表继续显示；空列表才整屏显示错误。
    let title = if error.is_some() && !songs.is_empty() {
        format!("{title} · 加载失败")
    } else {
        title.to_string()
    };
    let title = truncate_width(&title, area.width.saturating_sub(2) as usize);
    if area.height < 2 || area.width == 0 {
        return;
    }
    let (heading_area, inner) = list_split(area);
    frame.render_widget(Paragraph::new(heading_line(&title, theme)), heading_area);

    if let Some(err) = error
        && songs.is_empty()
    {
        draw_error(frame, inner, err, theme);
        return;
    }
    if songs.is_empty() {
        draw_message(
            frame,
            inner,
            empty_state_message(loading, empty),
            Style::default(),
        );
        return;
    }
    let selected_is_current = state
        .selected()
        .and_then(|index| songs.get(index))
        .is_some_and(|song| current_key.is_some_and(|key| key == song.key().as_str()));
    let table = song_table(songs, current_key, theme, inner.width, selected_is_current);
    frame.render_stateful_widget(table, inner, state);
}

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Cell, HighlightSpacing, Paragraph, Row, Table, TableState};

use musyc_core::model::Playlist;
use musyc_core::util::{human_count, truncate_width};

use crate::theme::Theme;
use crate::ui::list::{draw_error, draw_message, empty_state_message, heading_line, list_split};

fn playlist_table<'a>(playlists: &'a [Playlist], theme: &Theme, width: u16) -> Table<'a> {
    let symbol_w = 2usize;
    let spacing = 1usize;
    let idx_w = playlists.len().to_string().len().max(2);
    let track_w = playlists
        .iter()
        .map(|playlist| human_count(playlist.track_count).len())
        .max()
        .unwrap_or(4)
        .max(4);
    let show_creator = width as usize >= 72
        && playlists
            .iter()
            .any(|playlist| !playlist.creator.trim().is_empty());
    let total = width as usize;
    let base = symbol_w + idx_w + track_w;
    let content = total.saturating_sub(base + (if show_creator { 3 } else { 2 }) * spacing);
    let (name_w, creator_w) = if show_creator {
        let name = ((content * 62 / 100).max(12)).min(content);
        (name, content.saturating_sub(name))
    } else {
        (content, 0)
    };

    let rows = playlists.iter().enumerate().map(|(i, playlist)| {
        let mut cells = vec![
            Cell::from(format!("{:>idx_w$}", i + 1)),
            Cell::from(truncate_width(&playlist.name, name_w)),
        ];
        if show_creator {
            cells.push(Cell::from(truncate_width(&playlist.creator, creator_w)));
        }
        cells.push(Cell::from(format!(
            "{:>track_w$}",
            human_count(playlist.track_count)
        )));
        Row::new(cells).height(1)
    });

    let mut widths = vec![
        Constraint::Length(idx_w as u16),
        Constraint::Length(name_w as u16),
    ];
    if show_creator {
        widths.push(Constraint::Length(creator_w as u16));
    }
    widths.push(Constraint::Length(track_w as u16));

    Table::new(rows, widths)
        .column_spacing(spacing as u16)
        .highlight_symbol("  ")
        .highlight_spacing(HighlightSpacing::Always)
        .row_highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
}

#[allow(clippy::too_many_arguments)]
pub fn render_playlists(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    playlists: &[Playlist],
    state: &mut TableState,
    title: &str,
    loading: bool,
    error: Option<&str>,
    empty: &str,
) {
    let title = if error.is_some() && !playlists.is_empty() {
        format!("{title} · 加载失败")
    } else {
        title.to_string()
    };
    let title = truncate_width(&title, area.width.saturating_sub(2) as usize);
    if area.height < 2 || area.width == 0 {
        return;
    }
    // 标题行的 ‹ 与下方数字序号右对齐（idx_w：条目数位数，至少 2 列）。
    let idx_w = playlists.len().to_string().len().max(2);
    let (heading_area, inner) = list_split(area);
    frame.render_widget(
        Paragraph::new(heading_line(&title, idx_w, theme)),
        heading_area,
    );

    if let Some(err) = error
        && playlists.is_empty()
    {
        draw_error(frame, inner, err, theme);
        return;
    }
    if playlists.is_empty() {
        draw_message(
            frame,
            inner,
            empty_state_message(loading, empty),
            Style::default(),
        );
        return;
    }
    let table = playlist_table(playlists, theme, inner.width);
    frame.render_stateful_widget(table, inner, state);
}

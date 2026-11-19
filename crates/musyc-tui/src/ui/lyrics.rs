use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;

use musyc_core::lyric::Lyrics;

use crate::theme::Theme;
use crate::types::CoverArt;
use crate::ui::list::draw_message;

pub fn render_lyrics(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    lyrics: &Lyrics,
    pos_ms: u64,
    cover: Option<&CoverArt>,
    scroll_offset: &mut i32,
) {
    // 无边框：封面直接浮在内容区，歌词紧接右侧（fox 没有面板边框）。
    let inner = area;
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // 左边封面、右边歌词；面板太矮或太窄时只显示歌词。
    // 封面高度直接跟随面板内部高度缩放，不再绑定固定百分比。
    let show_cover = inner.height >= 5 && inner.width >= 40;
    let lyric_area = if show_cover {
        let _art_h = cover_art_height(inner.height);
        let cover_col = inner.width * 2 / 5; // 封面占 40%，歌词占 60%
        let chunks = Layout::horizontal([
            Constraint::Length(cover_col),
            Constraint::Length(2),
            Constraint::Min(10),
        ])
        .split(inner);
        render_cover(frame, chunks[0], theme, cover);
        chunks[2]
    } else {
        inner
    };

    if lyric_area.height == 0 || lyric_area.width == 0 {
        return;
    }
    if lyrics.is_empty() {
        draw_message(frame, lyric_area, "暂无歌词", Style::default());
        return;
    }

    // 先按歌词列宽度换行，再记录「当前逻辑行」对应的第一条显示行，
    // 这样自动滚动不会因为长行折行而错位。
    let width = lyric_area.width as usize;
    let current = lyrics.index_at(pos_ms);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_line = 0usize;
    for (i, lyric) in lyrics.lines.iter().enumerate() {
        let is_current = Some(i) == current;
        let style = if is_current {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            // 非当前行压暗，当前行是唯一焦点（fox 的高亮歌词同理）。
            Style::default()
        };
        for (part_index, part) in wrap_text(&lyric.text, width).into_iter().enumerate() {
            if is_current && part_index == 0 {
                current_line = lines.len();
            }
            lines.push(Line::from(Span::styled(part, style)));
        }
        if let Some(translation) = &lyric.translation {
            for part in wrap_text(translation, width) {
                lines.push(Line::from(Span::styled(
                    part,
                    Style::default().add_modifier(Modifier::ITALIC),
                )));
            }
        }
    }

    let height = lyric_area.height as usize;
    // 有时间轴时自动把当前行放在中间；纯文本从第一行开始。
    let base = if lyrics.timed {
        current_line.saturating_sub(height / 2)
    } else {
        0
    };
    let max_scroll = lines.len().saturating_sub(height);
    // 把偏移限制在当前可滚动范围内，避免连续翻页后积累大量无效偏移。
    let min_offset = -(base as i32);
    let max_offset = max_scroll as i32 - base as i32;
    let offset = (*scroll_offset).clamp(min_offset, max_offset);
    *scroll_offset = offset;
    let scroll = (base as i32 + offset) as usize;
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .scroll((scroll.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, lyric_area);
}

/// 按显示宽度换行，CJK 宽字符不会被从中间截断。
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() || width == 0 {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    let mut line = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used > 0 && used + w > width {
            out.push(std::mem::take(&mut line));
            used = 0;
        }
        line.push(ch);
        used += w;
    }
    if !line.is_empty() {
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// 封面高度跟随歌词面板内部高度变化。
const COVER_SCALE: f64 = 0.62;
/// 封面距面板左边的留白，以及右侧留给歌词列的空隙。
const COVER_LEFT_PAD: u16 = 5;
const COVER_RIGHT_PAD: u16 = 1;

fn cover_art_height(height: u16) -> u16 {
    let art = (height as f64 * COVER_SCALE).round() as u16;
    art.clamp(4, height.saturating_sub(1).max(4))
}

/// 用单色 braille 点阵把封面画成一幅抽象的小图，不再加边框。
fn render_cover(frame: &mut Frame, area: Rect, _theme: &Theme, cover: Option<&CoverArt>) {
    if area.width < 8 || area.height < 5 {
        return;
    }

    let aspect = cover
        .map(|art| art.width as f64 / art.height.max(1) as f64)
        .filter(|aspect| aspect.is_finite() && *aspect > 0.0)
        .unwrap_or(1.0);

    let left_pad = COVER_LEFT_PAD.min(area.width.saturating_sub(1));
    let max_art_w = area.width.saturating_sub(left_pad + COVER_RIGHT_PAD).max(1) as usize;
    let max_art_h = cover_art_height(area.height) as usize;

    // braille 一个字符承载 2x4 个点；终端字符高度约为宽度的两倍，
    // 因此保持视觉比例时字符宽度 = 2 * 字符高度 * 图片宽高比。
    let mut art_h = max_art_h;
    let mut art_w = (art_h as f64 * 2.0 * aspect).round().max(1.0) as usize;
    if art_w > max_art_w {
        art_w = max_art_w;
        art_h = (art_w as f64 / (2.0 * aspect)).round().max(1.0) as usize;
    }
    art_h = art_h.clamp(1, max_art_h);
    art_w = art_w.clamp(1, max_art_w);

    let rect = Rect {
        x: area.x + left_pad,
        y: area.y + (area.height - art_h as u16) / 2,
        width: art_w as u16,
        height: art_h as u16,
    };

    let Some(cover) = cover else {
        draw_message(frame, area, "暂无封面", Style::default());
        return;
    };

    frame.render_widget(
        Paragraph::new(braille_lines(cover, art_w, art_h, Style::default())),
        rect,
    );
}

/// 灰度 → 双线性缩放 → Otsu 阈值 → 2x4 braille。
fn braille_lines(cover: &CoverArt, cols: usize, rows: usize, style: Style) -> Vec<Line<'static>> {
    let src_w = cover.width as usize;
    let src_h = cover.height as usize;
    if src_w == 0 || src_h == 0 || cover.rgb.len() < src_w.saturating_mul(src_h).saturating_mul(3) {
        return Vec::new();
    }

    let gray: Vec<u8> = cover
        .rgb
        .chunks(3)
        .take(src_w * src_h)
        .map(|pixel| {
            (0.299 * pixel[0] as f64 + 0.587 * pixel[1] as f64 + 0.114 * pixel[2] as f64) as u8
        })
        .collect();

    let dst_w = cols * 2;
    let dst_h = rows * 4;
    let scaled = resize_gray(&gray, src_w, src_h, dst_w, dst_h);
    let threshold = otsu_threshold(&scaled);

    let mut lines = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut text = String::with_capacity(cols);
        for col in 0..cols {
            let mut bits = 0u32;
            for dy in 0..4 {
                for dx in 0..2 {
                    let px = col * 2 + dx;
                    let py = row * 4 + dy;
                    if scaled[py * dst_w + px] < threshold {
                        continue;
                    }
                    let dot = match (dx, dy) {
                        (0, 0) => 1,
                        (0, 1) => 2,
                        (0, 2) => 3,
                        (0, 3) => 7,
                        (1, 0) => 4,
                        (1, 1) => 5,
                        (1, 2) => 6,
                        (1, 3) => 8,
                        _ => 0,
                    };
                    bits |= 1 << (dot - 1);
                }
            }
            text.push(char::from_u32(0x2800 + bits).unwrap_or(' '));
        }
        lines.push(Line::from(Span::styled(text, style)));
    }
    lines
}

/// 把灰度图双线性缩放到点阵尺寸。
fn resize_gray(src: &[u8], src_w: usize, src_h: usize, dst_w: usize, dst_h: usize) -> Vec<u8> {
    let mut out = vec![0u8; dst_w * dst_h];
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return out;
    }
    for dy in 0..dst_h {
        let sy =
            ((dy as f64 + 0.5) * src_h as f64 / dst_h as f64 - 0.5).clamp(0.0, (src_h - 1) as f64);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(src_h - 1);
        let fy = sy - y0 as f64;
        for dx in 0..dst_w {
            let sx = ((dx as f64 + 0.5) * src_w as f64 / dst_w as f64 - 0.5)
                .clamp(0.0, (src_w - 1) as f64);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(src_w - 1);
            let fx = sx - x0 as f64;
            let top = src[y0 * src_w + x0] as f64 * (1.0 - fx) + src[y0 * src_w + x1] as f64 * fx;
            let bottom =
                src[y1 * src_w + x0] as f64 * (1.0 - fx) + src[y1 * src_w + x1] as f64 * fx;
            out[dy * dst_w + dx] = (top * (1.0 - fy) + bottom * fy).round() as u8;
        }
    }
    out
}

/// Otsu 自动阈值，避免深色 / 浅色封面在固定阈值下糊成一片。
fn otsu_threshold(gray: &[u8]) -> u8 {
    if gray.is_empty() {
        return 128;
    }
    let mut hist = [0u32; 256];
    for &value in gray {
        hist[value as usize] += 1;
    }
    let total = gray.len() as f64;
    let sum: f64 = (0..256).map(|i| i as f64 * hist[i] as f64).sum();

    let mut weight_bg = 0.0;
    let mut sum_bg = 0.0;
    let mut best = -1.0;
    let mut threshold = 128u8;
    for (t, &count) in hist.iter().enumerate() {
        weight_bg += count as f64;
        if weight_bg == 0.0 {
            continue;
        }
        let weight_fg = total - weight_bg;
        if weight_fg == 0.0 {
            break;
        }
        sum_bg += t as f64 * count as f64;
        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum - sum_bg) / weight_fg;
        let between = weight_bg * weight_fg * (mean_bg - mean_fg).powi(2);
        if between > best {
            best = between;
            threshold = t as u8;
        }
    }
    threshold
}

#[cfg(test)]
mod tests {
    use super::wrap_text;

    #[test]
    fn wraps_cjk_by_display_width() {
        assert_eq!(wrap_text("你好世界", 4), vec!["你好", "世界"]);
        assert_eq!(wrap_text("abc", 2), vec!["ab", "c"]);
    }

    #[test]
    fn does_not_split_wide_char() {
        assert_eq!(wrap_text("a你b", 2), vec!["a", "你", "b"]);
    }

    #[test]
    fn empty_text_keeps_one_line() {
        assert_eq!(wrap_text("", 4), vec![String::new()]);
    }
}

use ratatui::style::Color;

/// 极简配色：强调色青绿 #12aa9c，品牌色金莲花橙 #f86b1d，其余全部使用终端默认前景。
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    /// 金莲花橙 #f86b1d：首页 MUSYC 字标；列表当前播放行；播放条歌名。
    pub bender: Color,
}

pub const THEME: Theme = Theme {
    accent: Color::Rgb(18, 170, 156),
    bender: Color::Rgb(248, 107, 29),
};

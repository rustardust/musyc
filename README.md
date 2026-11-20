# musyc

[![CI](https://github.com/rustardust/musyc/actions/workflows/ci.yml/badge.svg)](https://github.com/rustardust/musyc/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rustardust/musyc)](https://github.com/rustardust/musyc/releases)
[![License: GPL v3](https://img.shields.io/badge/License-GPL_v3-blue.svg)](LICENSE)

> 基于歌单的终端音乐盒。

## 设计理念

**歌单，而不是曲库。** musyc 不试图做一个「什么都能搜到」的音乐平台。首页就是几张歌单：三个在线槽位，一个本地槽位。选一张，听下去。

**歌单可插拔。** 首页前三个在线入口不是写死的。光标停在入口上按 `s`，就能换成任意已加载的源——内置的网易原创、街声推荐、雀乐推荐，或自己写的 TOML 源。第四位固定为本地列表，第五位是最近播放。

**无登录，无解灰。** 不登录任何账号，不接第三方「解灰」音源。网易云只走官方直链与官方 30 秒试听；拿不到就跳过，不绕路。

**不执着于会员歌曲。** 除了会员曲库，还有太多新歌值得被发现。会员歌听不到就不听，把注意力留给那些可以自由获取的声音。

**极简主义。** 单一首页、五个入口、一个列表。`Esc` 逐级返回，并停在上次的位置；没有多层侧栏，没有冗余面板。

## 功能

- **五个入口**：网易原创 / 街声推荐 / 雀乐推荐 / 本地列表 / 最近播放，一屏直达。
- **网易云**：官方直链播放，官方 30 秒试听；支持单曲搜索、歌单搜索、链接 / ID / M3U 打开。无法播放自动跳过。
- **街声 / 雀乐**：经本机 API 转发（街声 HLS → MP3，雀乐直接取 CDN MP3），带歌词。
- **本地音乐**：递归扫描目录，按 `歌手 - 标题` 识别；`t` 转换选中、`T` 批量补全，先查网易云，再回退 MusicBrainz 与 LRCLIB，繁体先转简再匹配。**转换副本写入独立目录，原始文件不动**。
- **歌词**：默认关闭，`l` 开关；LRC 自动滚动，纯文本歌词按宽度换行；`v` 进入歌词纯享。
- **播放**：四种播放模式，进度 seek，音量记忆，退出保存队列与进度，下次启动停在原位置。
- **下载**：`d` 下载选中歌曲，自动写入标题 / 歌手 / 专辑 / 歌词 / 封面。
- **换源**：首页三个在线槽位可任意改绑，配置持久化。
- **MPRIS**：媒体键、`playerctl`、桌面媒体控件（Linux）。

### 快捷键

| 按键 | 作用 |
| --- | --- |
| `j` / `k`、`↓` / `↑` | 上下移动 |
| `g` / `G` | 列表首 / 尾 |
| `Enter` | 进入 / 播放 |
| `Esc` | 返回上一级 / 回首页 |
| `/` `f` `o` | 单曲搜索 / 歌单搜索 / 打开路径·链接·ID·M3U |
| `Space` | 播放 / 暂停 |
| `]` / `[` | 下一首 / 上一首 |
| `←` / `→` | 快退 / 快进 5 秒 |
| `+` / `-` | 音量 |
| `r` | 播放模式 |
| `l` / `v` | 歌词开关 / 歌词纯享 |
| `PageUp` / `PageDown` | 滚动歌词 |
| `t` / `T` | 转换选中 / 批量补全 |
| `x` / `Delete` | 删除最近播放记录 |
| `d` | 下载 |
| `R` | 刷新当前栏目 |
| `s` | 换源 |
| `?` | 快捷键帮助 |
| `q` | 退出 |

## 安装

```bash
git clone https://github.com/rustardust/musyc.git
cd musyc
./install.sh                 # 构建 release 并安装到 ~/.local/bin

cd api-server && npm ci && cd ..
./api-server/start.sh        # 本机 API，默认 127.0.0.1:3000
musyc
```

需要 Linux（PipeWire / PulseAudio）、Rust 1.88+、Node.js 18+、`ffmpeg` 在 `PATH` 中。编译依赖：Fedora 装 `alsa-lib-devel pkg-config`，Debian / Ubuntu 装 `libasound2-dev pkg-config`。

## 版权

本软件只是播放器，不拥有、不存储、不分发任何音乐作品。网络音频仅限个人试听：试听缓存与下载文件请在 **24 小时内删除**，禁止传播或商用。因使用本软件产生的版权纠纷由使用者自行承担。

版权所有 © 2026 rustardust，以 GPL-3.0-or-later 发布，见 [LICENSE](LICENSE)。接口能力来自 [netease-cloud-music-api-alger](https://github.com/algerkong/netease-cloud-music-api)，交互灵感来自 [go-musicfox](https://github.com/go-musicfox/go-musicfox) 与 [AlgerMusicPlayer](https://github.com/algerkong/AlgerMusicPlayer)。

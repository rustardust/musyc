# musyc for Android

musyc 的独立 Android 客户端：不依赖电脑上的 api-server，直接访问各平台公开接口。UI 用 Jetpack Compose（Material 3），播放用 Media3 / ExoPlayer。

## 功能

- 首页三个在线槽位：默认网易原创榜 / 街声推荐 / Audius 本周，可随时换源
- 可换的源：网易原创、街声推荐与分类、Audius、FMA、Jamendo，也可导入桌面端的自定义 `.toml`
- 雀乐推荐（按期浏览，18 期）
- 本地音乐（读取系统媒体库，只申请音频读取权限）；可补全标签：网易匹配后写入 MP3 的 ID3v2.3 副本（标题 / 歌手 / 专辑 / 封面 / 歌词），输出到 `Music/musyc`，原文件不动
- 最近播放（本机记录，最多 50 首，可逐条移除）
- 网易云单曲搜索与歌单搜索（歌单可打开全部曲目）
- 打开链接 / 歌曲 ID / 歌单 ID / 本地音频 / M3U
- 快退 / 快进 5 秒、音量调节（记忆）
- 播放：后台播放、通知栏 / 锁屏控制、自动跳过不可播放的曲子
- 播放模式：顺序 / 列表循环 / 单曲 / 随机，可在播放页切换并记忆
- 播放队列：播放页右下角打开，点任意一首直接切歌
- 歌词：全页歌词，LRC 当前行居中高亮 + 纯文本兜底
- 下载：直接音频保存到 `Download/musyc`（街声是 HLS 流，暂不支持下载）
- 定时关闭：15 / 30 / 45 / 60 / 90 分钟

设计理念与桌面版一致：歌单可插拔、无登录、无解灰、极简。

## 构建

需要 JDK 21 与 Android SDK（platform 35、build-tools 35.0.0）。

```bash
cd android
cp local.properties.example local.properties   # 或直接写入 sdk.dir=...
./gradlew :app:assembleDebug   # 或 :app:assembleRelease
```

产物：`app/build/outputs/apk/debug/app-debug.apk`

安装到已连接的设备：

```bash
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## 说明

- 仅使用各平台公开、无需登录的接口；拿不到地址的曲子（会员 / 版权限制）直接跳过，不做任何“解灰”。
- 街声播放地址是 HLS，交给 ExoPlayer 直接播放，不再像桌面端那样用 ffmpeg 转 MP3。
- 网易云直链有时效性，播放前按需解析。

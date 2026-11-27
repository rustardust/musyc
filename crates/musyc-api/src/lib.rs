//! 网易云接口客户端 + 音源解析。
//!
//! 只实现 `external` 后端：本机 netease-cloud-music-api。

pub mod client;
pub mod dto;
pub mod provider;
pub mod resolver;
pub mod source_loader;

pub use client::{ApiClient, RawLyric, SongUrl, StreetVoiceDetail};
pub use dto::IndiePlaylist;
pub use provider::{load_playlist, PlaylistRequest};
pub use resolver::{ResolvedSource, Resolver};
pub use source_loader::load as load_custom_source;

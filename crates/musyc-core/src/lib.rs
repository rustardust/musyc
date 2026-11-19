//! musyc 的核心：领域模型、配置与路径约定。

pub mod config;
pub mod error;
pub mod lyric;
pub mod model;
pub mod paths;
pub mod source_fetch;
pub mod sources;
pub mod util;

pub use config::{Config, UiConfig};
pub use error::{Error, Result};
pub use model::{PlayMode, Playlist, Quality, Song, SourceKind};
pub use paths::Paths;
pub use sources::{BuiltinSource, BUILTIN_SOURCES, Source, SourceKind as SourceDefinitionKind};

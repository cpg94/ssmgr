use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const DEFAULT_CATEGORIES: &[&str] = &[
    "drums",
    "bass",
    "synth",
    "vocal",
    "fx",
    "percussion",
    "melody",
    "ambient",
    "noise",
];

pub const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg", "aac", "m4a"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub duration_secs: Option<f64>,
    pub bpm: Option<f64>,
    pub categories: Vec<String>,
    pub enabled: bool,
    pub metadata: SampleMetadata,
    pub last_modified: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleMetadata {
    pub format: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: Option<u16>,
    pub tags: HashMap<String, String>,
}

impl Sample {
    pub fn new(path: String) -> Self {
        let name = std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let ext = std::path::Path::new(&path)
            .extension()
            .map(|s| s.to_string_lossy().to_string().to_uppercase())
            .unwrap_or_default();

        Self {
            id: Uuid::new_v4(),
            name,
            path,
            duration_secs: None,
            bpm: None,
            categories: Vec::new(),
            enabled: false,
            metadata: SampleMetadata {
                format: ext,
                sample_rate: 0,
                channels: 0,
                bit_depth: None,
                tags: HashMap::new(),
            },
            last_modified: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanDir {
    pub id: Uuid,
    pub path: String,
    pub label: String,
    pub added_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub scan_dirs: Vec<ScanDir>,
    pub port: u16,
    pub strudel_port: u16,
    pub samples: Vec<Sample>,
    pub last_scan: Option<u64>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            scan_dirs: Vec::new(),
            port: 8080,
            strudel_port: 8081,
            samples: Vec::new(),
            last_scan: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub server_url: String,
    pub playback_mode: PlaybackMode,
    pub samples: Vec<Sample>,
    pub last_sync: Option<u64>,
    pub search: String,
    pub selected_category: Option<String>,
    pub sort_by: SortBy,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:8080".to_string(),
            playback_mode: PlaybackMode::Once,
            samples: Vec::new(),
            last_sync: None,
            search: String::new(),
            selected_category: None,
            sort_by: SortBy::Name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaybackMode {
    Once,
    Loop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SortBy {
    Name,
    Bpm,
    Duration,
    Category,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrudelConfig {
    pub samples: HashMap<String, String>,
}

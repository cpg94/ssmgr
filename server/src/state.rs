use ssmgr_shared::{Sample, ScanDir, ServerConfig, StrudelConfig};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

const DEFAULT_CONFIG_PATH: &str = ".ssmgr/server.json";

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<ServerConfig>>,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn new(config_path: Option<String>) -> Self {
        let path = match config_path {
            Some(p) => PathBuf::from(p),
            None => {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(DEFAULT_CONFIG_PATH)
            }
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let config = load_config(&path).unwrap_or_default();
        info!("Loaded config with {} samples", config.samples.len());

        Self {
            config: Arc::new(RwLock::new(config)),
            config_path: path,
        }
    }

    pub async fn save(&self) {
        let config = self.config.read().await;
        if let Err(e) = save_config(&self.config_path, &config) {
            warn!("Failed to save config: {}", e);
        }
    }

    pub async fn get_samples(&self) -> Vec<Sample> {
        self.config.read().await.samples.clone()
    }

    pub async fn filter_samples(
        &self,
        search: Option<&str>,
        category: Option<&str>,
        enabled: Option<bool>,
    ) -> Vec<Sample> {
        let config = self.config.read().await;
        config
            .samples
            .iter()
            .filter(|s| {
                if let Some(en) = enabled {
                    if s.enabled != en {
                        return false;
                    }
                }
                if let Some(cat) = category {
                    if !s.categories.iter().any(|c| c == cat) {
                        return false;
                    }
                }
                if let Some(q) = search {
                    if !s.name.to_lowercase().contains(&q.to_lowercase())
                        && !s.path.to_lowercase().contains(&q.to_lowercase())
                    {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub async fn toggle_sample(&self, id: uuid::Uuid) -> Option<bool> {
        let mut config = self.config.write().await;
        if let Some(sample) = config.samples.iter_mut().find(|s| s.id == id) {
            sample.enabled = !sample.enabled;
            let enabled = sample.enabled;
            drop(config);
            self.save().await;
            Some(enabled)
        } else {
            None
        }
    }

    pub async fn add_category(&self, id: uuid::Uuid, category: String) -> bool {
        let mut config = self.config.write().await;
        if let Some(sample) = config.samples.iter_mut().find(|s| s.id == id) {
            if !sample.categories.contains(&category) {
                sample.categories.push(category);
            }
            drop(config);
            self.save().await;
            true
        } else {
            false
        }
    }

    pub async fn remove_category(&self, id: uuid::Uuid, category: &str) -> bool {
        let mut config = self.config.write().await;
        if let Some(sample) = config.samples.iter_mut().find(|s| s.id == id) {
            sample.categories.retain(|c| c != category);
            drop(config);
            self.save().await;
            true
        } else {
            false
        }
    }

    pub async fn add_scan_dir(&self, path: String, label: String) {
        let mut config = self.config.write().await;
        let scan_dir = ScanDir {
            id: uuid::Uuid::new_v4(),
            path,
            label,
            added_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        config.scan_dirs.push(scan_dir);
        drop(config);
        self.save().await;
    }

    pub async fn remove_scan_dir(&self, id: uuid::Uuid) {
        let mut config = self.config.write().await;
        config.scan_dirs.retain(|d| d.id != id);
        drop(config);
        self.save().await;
    }

    pub async fn rescan(&self) -> (Vec<Sample>, Vec<Sample>, Vec<Sample>) {
        let directories = {
            let config = self.config.read().await;
            config.scan_dirs.clone()
        };

        let result = {
            let mut config = self.config.write().await;
            let (added, updated, removed) =
                crate::scanner::incremental_scan(&mut config.samples, &directories);
            config.last_scan = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            );
            (added, updated, removed)
        };

        self.save().await;
        result
    }

    pub async fn analyze_sample_bpm(&self, id: uuid::Uuid) -> Option<f64> {
        let path = {
            let config = self.config.read().await;
            config.samples.iter().find(|s| s.id == id).map(|s| s.path.clone())
        };

        if let Some(path) = path {
            let bpm = crate::analyze::analyze_bpm(&path);
            if let Some(bpm_val) = bpm {
                let mut config = self.config.write().await;
                if let Some(sample) = config.samples.iter_mut().find(|s| s.id == id) {
                    sample.bpm = Some(bpm_val);
                }
                drop(config);
                self.save().await;
            }
            bpm
        } else {
            None
        }
    }

    pub async fn generate_strudel_config(&self) -> StrudelConfig {
        let config = self.config.read().await;
        let mut samples = HashMap::new();

        for sample in &config.samples {
            if sample.enabled {
                let name = sample.name.clone();
                let url = format!(
                    "http://localhost:{}/samples/{}",
                    config.strudel_port,
                    std::path::Path::new(&sample.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_default()
                );
                samples.insert(name, url);
            }
        }

        StrudelConfig { samples }
    }
}

fn load_config(path: &Path) -> Option<ServerConfig> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_config(path: &Path, config: &ServerConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

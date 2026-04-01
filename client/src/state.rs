use ssmgr_shared::Sample;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

const DEFAULT_CONFIG_PATH: &str = ".ssmgr/client.json";

#[derive(Clone)]
pub struct ClientState {
    pub config: Arc<RwLock<ClientConfig>>,
    pub config_path: PathBuf,
    pub server_connected: Arc<RwLock<bool>>,
}

impl ClientState {
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
        info!("Loaded client config with {} cached samples", config.samples.len());

        Self {
            config: Arc::new(RwLock::new(config)),
            config_path: path,
            server_connected: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn save(&self) {
        let config = self.config.read().await;
        if let Err(e) = save_config(&self.config_path, &config) {
            tracing::warn!("Failed to save config: {}", e);
        }
    }

    pub async fn set_samples(&self, samples: Vec<Sample>) {
        let mut config = self.config.write().await;
        config.samples = samples;
        config.last_sync = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        );
        drop(config);
        self.save().await;
    }

    pub async fn get_filtered_samples(&self) -> Vec<Sample> {
        let config = self.config.read().await;
        let search = config.search.clone();
        let category = config.selected_category.clone();
        let samples = config.samples.clone();
        drop(config);

        samples
            .into_iter()
            .filter(|s| {
                if let Some(ref cat) = category {
                    if !s.categories.iter().any(|c| c == cat) {
                        return false;
                    }
                }
                if !search.is_empty() {
                    let q = search.to_lowercase();
                    if !s.name.to_lowercase().contains(&q)
                        && !s.path.to_lowercase().contains(&q)
                    {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    pub async fn set_search(&self, search: String) {
        let mut config = self.config.write().await;
        config.search = search;
    }

    pub async fn set_category(&self, category: Option<String>) {
        let mut config = self.config.write().await;
        config.selected_category = category;
    }

    pub async fn get_all_categories(&self) -> Vec<String> {
        let config = self.config.read().await;
        let mut cats: Vec<String> = config
            .samples
            .iter()
            .flat_map(|s| s.categories.clone())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    }

    pub async fn get_server_url(&self) -> String {
        self.config.read().await.server_url.clone()
    }
}

use ssmgr_shared::ClientConfig;

fn load_config(path: &std::path::Path) -> Option<ClientConfig> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_config(path: &std::path::Path, config: &ClientConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

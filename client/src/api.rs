use reqwest::Client;
use ssmgr_shared::{ApiResponse, Sample, ScanDir, StrudelConfig};
use std::collections::HashMap;
use std::time::Duration;
use tracing::warn;

pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, base_url }
    }

    pub fn update_base_url(&mut self, url: String) {
        self.base_url = url;
    }

    pub async fn health_check(&self) -> bool {
        self.get_samples(None, None, None).await.is_ok()
    }

    pub async fn get_samples(
        &self,
        search: Option<&str>,
        category: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<Vec<Sample>, String> {
        let mut url = format!("{}/api/samples", self.base_url);
        let mut params: Vec<(&str, String)> = Vec::new();

        if let Some(s) = search {
            params.push(("search", s.to_string()));
        }
        if let Some(c) = category {
            params.push(("category", c.to_string()));
        }
        if let Some(e) = enabled {
            params.push(("enabled", e.to_string()));
        }

        if !params.is_empty() {
            let query: String = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            url = format!("{}?{}", url, query);
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<Vec<Sample>> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or_default())
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn toggle_sample(&self, id: &str) -> Result<bool, String> {
        let url = format!("{}/api/samples/{}/toggle", self.base_url, id);
        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<bool> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or(false))
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn add_category(&self, id: &str, category: &str) -> Result<bool, String> {
        let url = format!("{}/api/samples/{}/category", self.base_url, id);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "category": category }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<bool> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or(false))
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn remove_category(&self, id: &str, category: &str) -> Result<bool, String> {
        let url = format!("{}/api/samples/{}/category", self.base_url, id);
        let resp = self
            .client
            .delete(&url)
            .json(&serde_json::json!({ "category": category }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<bool> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or(false))
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn analyze_sample(&self, id: &str) -> Result<f64, String> {
        let url = format!("{}/api/samples/{}/analyze", self.base_url, id);
        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<f64> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or(0.0))
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn get_scan_dirs(&self) -> Result<Vec<ScanDir>, String> {
        let url = format!("{}/api/scan-dirs", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<Vec<ScanDir>> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or_default())
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn add_scan_dir(&self, path: &str, label: &str) -> Result<bool, String> {
        let url = format!("{}/api/scan-dirs", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "path": path, "label": label }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<bool> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or(false))
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn remove_scan_dir(&self, id: &str) -> Result<bool, String> {
        let url = format!("{}/api/scan-dirs/{}", self.base_url, id);
        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<bool> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or(false))
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn rescan(&self) -> Result<HashMap<String, usize>, String> {
        let url = format!("{}/api/rescan", self.base_url);
        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<HashMap<String, usize>> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or_default())
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn get_strudel_config(&self) -> Result<StrudelConfig, String> {
        let url = format!("{}/api/strudel.json", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let api_resp: ApiResponse<StrudelConfig> =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;

        if api_resp.success {
            Ok(api_resp.data.unwrap_or(StrudelConfig {
                samples: HashMap::new(),
            }))
        } else {
            Err(api_resp.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub fn get_audio_url(&self, sample_path: &str) -> String {
        let filename = std::path::Path::new(sample_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        format!("{}/samples/{}", self.base_url, filename)
    }
}

use ssmgr_shared::{Sample, ScanDir, SUPPORTED_EXTENSIONS};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use walkdir::WalkDir;

pub fn scan_directory(dir: &ScanDir) -> Vec<Sample> {
    info!("Scanning directory: {}", dir.path);
    let mut samples = Vec::new();

    for entry in WalkDir::new(&dir.path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if !is_supported(path) {
            continue;
        }

        let path_str = path.to_string_lossy().to_string();
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to read metadata for {}: {}", path_str, e);
                continue;
            }
        };

        let last_modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut sample = Sample::new(path_str);
        sample.last_modified = last_modified;

        if let Some(meta) = crate::analyze::extract_metadata(&sample.path) {
            sample.metadata = meta;
        }

        if let Some(duration) = crate::analyze::get_duration(&sample.path) {
            sample.duration_secs = Some(duration);
        }

        samples.push(sample);
    }

    info!("Found {} samples in {}", samples.len(), dir.path);
    samples
}

pub fn scan_all(directories: &[ScanDir]) -> Vec<Sample> {
    directories.iter().flat_map(scan_directory).collect()
}

fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn incremental_scan(
    existing: &mut Vec<Sample>,
    directories: &[ScanDir],
) -> (Vec<Sample>, Vec<Sample>, Vec<Sample>) {
    let new_samples = scan_all(directories);
    let mut existing_map: std::collections::HashMap<String, Sample> =
        existing.drain(..).map(|s| (s.path.clone(), s)).collect();

    let mut added = Vec::new();
    let mut updated = Vec::new();

    for new in new_samples {
        if let Some(old) = existing_map.remove(&new.path) {
            if new.last_modified > old.last_modified {
                let mut updated_sample = new;
                updated_sample.id = old.id;
                updated_sample.enabled = old.enabled;
                updated_sample.categories = old.categories;
                updated_sample.bpm = old.bpm;
                updated.push(updated_sample.clone());
                existing.push(updated_sample);
            } else {
                existing.push(old);
            }
        } else {
            added.push(new.clone());
            existing.push(new);
        }
    }

    let removed: Vec<Sample> = existing_map.into_values().collect();

    (added, updated, removed)
}

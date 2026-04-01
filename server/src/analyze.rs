use ssmgr_shared::SampleMetadata;
use std::collections::HashMap;
use std::path::Path;
use symphonia::core::audio::{AudioBuffer, AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tracing::info;

pub fn extract_metadata(path: &str) -> Option<SampleMetadata> {
    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(0);
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count() as u16)
        .unwrap_or(0);

    let bit_depth = track.codec_params.bits_per_sample.map(|b| b as u16);

    let mut tags = HashMap::new();
    if let Some(metadata) = format.metadata().current() {
        for tag in metadata.tags() {
            if let symphonia::core::meta::Value::String(ref value) = tag.value {
                tags.insert(tag.key.to_string(), value.clone());
            }
        }
    }

    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string().to_uppercase())
        .unwrap_or_default();

    Some(SampleMetadata {
        format: ext,
        sample_rate,
        channels,
        bit_depth,
        tags,
    })
}

fn collect_samples(buf: &AudioBufferRef, samples: &mut Vec<f32>, max: usize) {
    let n_frames = buf.frames();
    let n_channels = buf.spec().channels.count();

    let mut f32_buf = AudioBuffer::<f32>::new(n_frames as u64, *buf.spec());
    buf.convert(&mut f32_buf);

    for f in 0..n_frames {
        for ch in 0..n_channels {
            if samples.len() >= max {
                return;
            }
            samples.push(f32_buf.chan(ch)[f]);
        }
    }
}

pub fn analyze_bpm(path: &str) -> Option<f64> {
    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;

    let mut format = probed.format;
    let track_ref = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)?;
    let track_id = track_ref.id;
    let sample_rate = track_ref.codec_params.sample_rate.unwrap_or(44100);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track_ref.codec_params, &DecoderOptions::default())
        .ok()?;

    let mut samples: Vec<f32> = Vec::new();
    let max_samples = (sample_rate as usize) * 30;

    while let Ok(packet) = format.next_packet() {
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder.decode(&packet).ok()?;
        let buf: AudioBufferRef = decoded;
        collect_samples(&buf, &mut samples, max_samples);

        if samples.len() >= max_samples {
            break;
        }
    }

    if samples.len() < 4410 {
        return None;
    }

    let bpm = estimate_bpm(&samples, sample_rate);
    info!("BPM analysis for {}: {:.1}", path, bpm);
    Some(bpm)
}

fn estimate_bpm(samples: &[f32], sample_rate: u32) -> f64 {
    let window_size = sample_rate as usize;
    let step = sample_rate as usize / 4;
    let mut bpms: Vec<f64> = Vec::new();

    for start in (0..samples.len().saturating_sub(window_size)).step_by(step) {
        let window = &samples[start..start + window_size];
        if let Some(bpm) = detect_bpm_onset(window, sample_rate) {
            bpms.push(bpm);
        }
    }

    if bpms.is_empty() {
        return 0.0;
    }

    bpms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = bpms[bpms.len() / 2];

    if median < 60.0 {
        median * 2.0
    } else if median > 200.0 {
        median / 2.0
    } else {
        median
    }
}

fn detect_bpm_onset(samples: &[f32], sample_rate: u32) -> Option<f64> {
    let hop = sample_rate / 100;
    let mut energy: Vec<f32> = Vec::new();

    for chunk in samples.chunks(hop as usize) {
        let e: f32 = chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32;
        energy.push(e);
    }

    if energy.len() < 4 {
        return None;
    }

    let mut diff: Vec<f32> = Vec::new();
    for i in 1..energy.len() {
        let d = energy[i] - energy[i - 1];
        if d > 0.0 {
            diff.push(d);
        } else {
            diff.push(0.0);
        }
    }

    let threshold = diff.iter().sum::<f32>() / diff.len() as f32 * 1.5;
    let mut onsets: Vec<usize> = Vec::new();

    for i in 1..diff.len().saturating_sub(1) {
        if diff[i] > threshold && diff[i] > diff[i - 1] && diff[i] > diff[i + 1] {
            onsets.push(i);
        }
    }

    if onsets.len() < 2 {
        return None;
    }

    let intervals: Vec<f64> = onsets
        .windows(2)
        .map(|w| (w[1] - w[0]) as f64 * hop as f64 / sample_rate as f64)
        .collect();

    let avg_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;

    if avg_interval <= 0.0 {
        return None;
    }

    Some(60.0 / avg_interval)
}

pub fn get_duration(path: &str) -> Option<f64> {
    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;

    let track = probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)?;

    let n_frames = track.codec_params.n_frames?;
    let sample_rate = track.codec_params.sample_rate? as f64;

    Some(n_frames as f64 / sample_rate)
}

pub fn is_supported_audio(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| ssmgr_shared::SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

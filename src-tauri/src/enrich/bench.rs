//! Cold-start batch benchmark: load a model from nothing and label a burst of
//! notes, the way a debounced queue would. Run it under an external sampler
//! for CPU and memory; this side only prints phase markers.
//!
//! `SN_BENCH_MODEL=<path to .gguf> cargo test bench_cold_batch -- --ignored --nocapture`

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::benchmark::SAMPLES as NOTES;
use super::language::ENGLISH;
use super::llama::LlamaCpp;
use super::model::Backend;
use super::runner::enrich;

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn mark(phase: &str, extra: serde_json::Value) {
    eprintln!(
        "BENCH {}",
        serde_json::json!({ "t": now_ms() as u64, "phase": phase, "extra": extra })
    );
}

#[test]
#[ignore = "needs a model and takes minutes; run under the sampler"]
fn bench_cold_batch() {
    let path = std::path::PathBuf::from(
        std::env::var("SN_BENCH_MODEL").expect("set SN_BENCH_MODEL to a .gguf path"),
    );

    // The same categories the app would send, so the prompt is realistic.
    let home = std::env::var_os("USERPROFILE").expect("USERPROFILE");
    let root = std::path::PathBuf::from(home).join("Scratchnote");
    let categories = crate::storage::categories::load(&root);

    // A quiet stretch first, so the sampler sees the baseline.
    mark(
        "start",
        serde_json::json!({
            "model": path.display().to_string(),
            "gpus": super::llama::gpu_devices(),
        }),
    );
    std::thread::sleep(std::time::Duration::from_secs(2));

    mark("load_begin", serde_json::json!({}));
    let started = Instant::now();
    // SN_BENCH_GPU=0 measures the CPU path, as the settings toggle does.
    let use_gpu = std::env::var("SN_BENCH_GPU").map_or(true, |v| v != "0");
    let backend: Box<dyn Backend> =
        Box::new(LlamaCpp::load_with(&path, use_gpu).expect("model loads"));
    mark(
        "load_end",
        serde_json::json!({ "ms": started.elapsed().as_millis() as u64 }),
    );

    for (i, note) in NOTES.iter().enumerate() {
        let started = Instant::now();
        let result = enrich(note, ENGLISH, &categories, backend.as_ref());
        mark(
            "note",
            serde_json::json!({
                "i": i,
                "words": note.split_whitespace().count(),
                "ms": started.elapsed().as_millis() as u64,
                "ok": result.is_ok(),
                "category": result.map(|e| e.category).unwrap_or_default(),
            }),
        );
    }

    mark("batch_end", serde_json::json!({}));
    drop(backend);
    mark("dropped", serde_json::json!({}));
    // Long enough for the sampler to see whether the memory comes back.
    std::thread::sleep(std::time::Duration::from_secs(3));
    mark("end", serde_json::json!({}));
}

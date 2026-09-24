//! Cold-start batch benchmark: load a model from nothing and label a burst of
//! notes, the way a debounced queue would. Run it under an external sampler
//! for CPU and memory; this side only prints phase markers.
//!
//! `SN_BENCH_MODEL=<path to .gguf> cargo test bench_cold_batch -- --ignored --nocapture`

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::llama::LlamaCpp;
use super::model::Backend;
use super::normalize::Vocabulary;
use super::runner::enrich;

const NOTES: [&str; 20] = [
    "Buy a new USB-C hub for the homelab, the current one drops the ethernet link",
    "ArgoCD auto-sync broke staging again. Pin the chart version and roll back before Friday's release.",
    "Réunion avec l'équipe infra demain à 10h pour parler de la migration Kubernetes",
    "Idea: Scratchnote could show a weekly digest of tags",
    "Call the dentist to move the appointment to next Tuesday",
    "Read the llama.cpp docs on KV cache quantisation, might cut memory in half for long contexts",
    "The PrimeNG table component re-renders every row on sort, which is why the reporting page lags with 5k rows. Try virtual scroll or trackBy.",
    "Penser à renouveler le passeport avant l'été",
    "Datadog monitor for the bidder p99 latency keeps flapping, raise the evaluation window to 10 minutes",
    "Book train tickets Paris to Lyon for the 14th",
    "Balise sync failed overnight with a 401 from the Confluence API; the token probably expired",
    "Refactor the tag normalisation so manual tags skip the fuzzy merge",
    "Grocery: oat milk, coffee beans, lemons, parmesan",
    "Snowflake query on BIDREQUEST_INGEST_INITIAL takes 4 minutes, add a date filter on the partition column first",
    "Watch the Rust talk about async cancellation safety",
    "La démo client est décalée à jeudi, prévenir l'équipe produit",
    "Tauri global shortcut does not fire when a fullscreen game has focus; document it as a known limitation",
    "Try the Light model for a week and compare tag quality with the 4B one",
    "Postmortem notes: the rollback took 40 minutes because nobody had the prod kubeconfig. Store it in the vault and add a runbook step.",
    "Remember to water the plants on the balcony",
];

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

    // The same vocabulary the app would send, so the prompt is realistic.
    let home = std::env::var_os("USERPROFILE").expect("USERPROFILE");
    let root = std::path::PathBuf::from(home).join("Scratchnote");
    let vocabulary = Vocabulary {
        counts: crate::storage::index::rebuild(&root).tag_counts(),
        aliases: crate::storage::tags::load_aliases(&root),
        categories: crate::storage::categories::load(&root),
    };

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
        let result = enrich(note, &vocabulary, backend.as_ref());
        mark(
            "note",
            serde_json::json!({
                "i": i,
                "words": note.split_whitespace().count(),
                "ms": started.elapsed().as_millis() as u64,
                "ok": result.is_ok(),
                "tags": result.map(|e| e.tags).unwrap_or_default(),
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

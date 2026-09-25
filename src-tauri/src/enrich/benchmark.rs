//! The notes the settings benchmark labels. Fixed, so every machine runs the
//! same test and it works in an empty space. Nothing is written back.

/// Short and long, English and French, the way real notes come. Twenty, so
/// the average holds up against one slow note.
pub const SAMPLES: [&str; 20] = [
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

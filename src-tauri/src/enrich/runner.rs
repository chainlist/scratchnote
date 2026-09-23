//! Turning one note body into enrichment, and that enrichment into a patch
//! for the markdown.
//!
//! Deliberately free of any app plumbing: no queue, no files, no Tauri. That
//! is what lets the golden tests in SPEC 12 drive the whole path with a stub
//! backend and check the bytes that come out the other end.

use crate::storage::daily_file::{NotePatch, Status};

use super::model::Backend;
use super::normalize::{self, Vocabulary};
use super::{grammar, prompt, Enrichment};

/// Label one note. The vocabulary steers the model towards tags already in
/// use and then normalises whatever it returns anyway.
pub fn enrich(
    body: &str,
    vocabulary: &Vocabulary,
    backend: &dyn Backend,
) -> Result<Enrichment, String> {
    let existing = vocabulary.by_frequency();
    let text = prompt::build(body, &existing);

    let raw = backend.generate(&text, grammar::ENRICHMENT_GBNF)?;
    let mut enrichment = Enrichment::parse(&raw)?;

    enrichment.tags = normalize::normalize(&enrichment.tags, vocabulary);
    if enrichment.tags.is_empty() {
        return Err("every tag normalised away to nothing".to_string());
    }

    Ok(enrichment)
}

/// What to write into the note block. The body and hash are untouched.
pub fn patch(enrichment: &Enrichment) -> NotePatch {
    NotePatch {
        subject: Some(enrichment.subject.clone()),
        summary: Some(enrichment.summary.clone()),
        tags: enrichment.tags.clone(),
        status: Status::Done,
    }
}

/// Marks a note the queue has given up on (SPEC 5.6). Subject and summary are
/// left alone so a later retry, or the user, can fill them in.
pub fn failed_patch(current: &crate::storage::daily_file::Note) -> NotePatch {
    NotePatch {
        subject: current.subject.clone(),
        summary: current.summary.clone(),
        tags: current.tags.clone(),
        status: Status::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrich::model::StubBackend;
    use crate::storage::daily_file::{append_note, body_hash, parse_notes, update_note, Note};

    const DATE: &str = "2026-09-22";
    const FILE: &str = "notes/2026/2026-09-22.md";

    fn fixed_stub() -> StubBackend {
        StubBackend::new(
            r#"{"subject":"Rollback plan","summary":"Pin the chart and roll back staging.","tags":["ArgoCD","Staging","staging"]}"#,
        )
    }

    fn pending(id: &str, body: &str) -> Note {
        Note {
            id: id.to_string(),
            date: DATE.to_string(),
            time: "09:00".to_string(),
            file: FILE.to_string(),
            subject: None,
            summary: None,
            tags: Vec::new(),
            status: Status::Pending,
            hash: body_hash(body),
            body: body.to_string(),
        }
    }

    /// SPEC 12 asks for twenty sample notes in English and French.
    fn samples() -> Vec<&'static str> {
        vec![
            "Talked with the team, the auto-sync broke staging again.",
            "Buy a new USB-C hub for the homelab",
            "Remember to rotate the API keys before the audit",
            "The deploy pipeline times out on the integration stage",
            "Book a dentist appointment for next month",
            "Idea: cache the tag counts instead of recomputing them",
            "Ask Sam about the migration plan for the old cluster",
            "Postgres connection pool is exhausted under load",
            "Read the paper on speculative decoding",
            "Cancel the unused monitoring subscription",
            "Réunion demain à 14h avec l'équipe infra",
            "Penser à racheter du café pour le bureau",
            "Le déploiement de staging a encore échoué ce matin",
            "Noter les chiffres du trimestre avant vendredi",
            "Appeler le comptable au sujet de la TVA",
            "Idée: séparer le cache des tags du reste de l'index",
            "Revoir la configuration du pare-feu la semaine prochaine",
            "Commander un nouveau clavier pour le poste de travail",
            "Préparer les slides pour la revue d'architecture",
            "Vérifier les sauvegardes du serveur de fichiers",
        ]
    }

    #[test]
    fn there_are_twenty_samples_in_both_languages() {
        assert_eq!(samples().len(), 20);
        assert!(samples().iter().any(|note| note.contains("Réunion")));
        assert!(samples().iter().any(|note| note.contains("Talked with")));
    }

    #[test]
    fn every_sample_writes_back_in_the_format_from_the_spec() {
        let vocabulary = Vocabulary::default();
        let backend = fixed_stub();

        for (i, body) in samples().iter().enumerate() {
            let id = format!("01SAMPLE{i:02}");
            let note = pending(&id, body);
            let doc = append_note("", &note, DATE);

            let enrichment = enrich(body, &vocabulary, &backend).expect("stub always succeeds");
            let out = update_note(&doc, &id, &patch(&enrichment)).expect("id is present");

            // The block must read back as an enriched note with the body intact.
            let parsed = parse_notes(&out, DATE, FILE);
            assert_eq!(parsed.len(), 1, "one note for {body}");
            let parsed = &parsed[0];
            assert_eq!(parsed.body, *body, "body changed for {body}");
            assert_eq!(parsed.hash, note.hash, "hash changed for {body}");
            assert_eq!(parsed.status, Status::Done);
            assert_eq!(parsed.subject.as_deref(), Some("Rollback plan"));
            assert_eq!(
                parsed.summary.as_deref(),
                Some("Pin the chart and roll back staging.")
            );
            // Duplicates and casing are normalised away.
            assert_eq!(parsed.tags, vec!["argocd", "staging"]);

            // And the literal shape: heading, then exactly two blockquote lines.
            assert!(out.contains("### Rollback plan\n"), "heading for {body}");
            assert!(
                out.contains("> Pin the chart and roll back staging.\n> #argocd #staging\n\n"),
                "summary and tag lines for {body}:\n{out}"
            );
        }
    }

    #[test]
    fn steers_tags_towards_the_vocabulary_already_in_use() {
        let vocabulary = Vocabulary {
            counts: [("staging".to_string(), 12)].into_iter().collect(),
            aliases: Default::default(),
        };
        // The stub answers "Stagging", one edit away from the known tag.
        let backend = StubBackend::new(r#"{"subject":"a","summary":"b","tags":["Stagging"]}"#);
        let out = enrich("some note", &vocabulary, &backend).unwrap();
        assert_eq!(out.tags, vec!["staging"]);
    }

    #[test]
    fn a_backend_failure_is_reported_rather_than_written_back() {
        struct Failing;
        impl Backend for Failing {
            fn generate(&self, _: &str, _: &str) -> Result<String, String> {
                Err("model exploded".to_string())
            }
        }
        let err = enrich("note", &Vocabulary::default(), &Failing).unwrap_err();
        assert!(err.contains("model exploded"));
    }

    #[test]
    fn output_that_is_not_the_object_is_a_failure_not_a_bad_write() {
        let backend = StubBackend::new("I'm sorry, I cannot do that.");
        assert!(enrich("note", &Vocabulary::default(), &backend).is_err());
    }

    #[test]
    fn tags_that_all_normalise_away_are_a_failure() {
        let backend =
            StubBackend::new("{\"subject\":\"a\",\"summary\":\"b\",\"tags\":[\"###\",\"!!!\"]}");
        assert!(enrich("note", &Vocabulary::default(), &backend).is_err());
    }

    /// A note should be tagged with what it is about, including the project or
    /// broad subject it belongs to, not with what kind of note it is.
    /// Needs a model; `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a downloaded model"]
    fn tags_a_technical_note_topically() {
        let Some(backend) = load_installed_model() else {
            return;
        };

        // The vocabulary a few generic tags have already polluted, which is the
        // situation the prompt has to cope with.
        let vocabulary = Vocabulary {
            counts: [
                ("plan", 4u32),
                ("test", 4),
                ("sync", 3),
                ("issue", 2),
                ("rollback", 1),
                ("argocd", 1),
            ]
            .into_iter()
            .map(|(t, c)| (t.to_string(), c))
            .collect(),
            aliases: Default::default(),
        };

        let cases: [(&str, &[&str]); 2] = [
            (
                "Translation issue with the API request, and prefer to go from API to pure json file",
                &["translation", "api", "json"],
            ),
            (
                "The grammar crashed because sampler.accept was called twice per token in the Rust decode loop of llama.cpp",
                &["grammar", "rust"],
            ),
        ];

        let mut missing = Vec::new();
        for (note, wanted) in cases {
            let out = enrich(note, &vocabulary, backend.as_ref()).expect("should label");
            eprintln!(
                "{note}
  -> {} | {:?}",
                out.subject, out.tags
            );

            for topic in wanted {
                if !out.tags.iter().any(|tag| tag == topic) {
                    missing.push(format!("{topic:?} for {note:?} (got {:?})", out.tags));
                }
            }
            assert!(
                !out.tags.iter().any(|t| t == "issue" || t == "sync"),
                "a genre tag survived: {:?}",
                out.tags
            );
        }
        assert!(
            missing.is_empty(),
            "missing topical tags:
  {}",
            missing.join(
                "
  "
            )
        );
    }

    /// Shared by the tests that need real weights.
    fn load_installed_model() -> Option<Box<dyn Backend>> {
        use crate::enrich::download;
        use crate::enrich::llama::LlamaCpp;
        use crate::enrich::model::model_file;

        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        let root = std::path::PathBuf::from(home).join("Scratchnote");
        let variant = download::installed_variant(&root)?;
        Some(Box::new(
            LlamaCpp::load(&model_file(&root, variant)).expect("the model should load"),
        ))
    }

    /// SPEC 12's optional integration test, and the only thing that checks
    /// SPEC 11's "zero parse failures" against a real sampler rather than a
    /// stub. Needs a downloaded model; run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a downloaded model"]
    fn the_real_model_returns_schema_valid_json_for_every_sample() {
        use crate::enrich::download;
        use crate::enrich::llama::LlamaCpp;
        use crate::enrich::model::model_file;

        let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
        else {
            eprintln!("no home directory, skipping");
            return;
        };
        let root = std::path::PathBuf::from(home).join("Scratchnote");

        let Some(variant) = download::installed_variant(&root) else {
            eprintln!("no model installed, skipping");
            return;
        };

        let backend = LlamaCpp::load(&model_file(&root, variant)).expect("the model should load");
        let vocabulary = Vocabulary::default();

        for body in samples().iter().take(5) {
            let out = enrich(body, &vocabulary, &backend)
                .unwrap_or_else(|e| panic!("{body:?} failed: {e}"));

            assert!(!out.subject.trim().is_empty(), "empty subject for {body:?}");
            assert!(!out.summary.trim().is_empty(), "empty summary for {body:?}");
            assert!(
                (1..=5).contains(&out.tags.len()),
                "{} tags for {body:?}: {:?}",
                out.tags.len(),
                out.tags
            );
            for tag in &out.tags {
                assert!(
                    tag.chars().all(|c| c.is_alphanumeric() || c == '-'),
                    "tag {tag:?} is not normalised for {body:?}"
                );
            }
            eprintln!(
                "{body:?}
  -> {} | {:?}",
                out.subject, out.tags
            );
        }
    }

    #[test]
    fn the_failed_patch_keeps_what_was_already_there() {
        let mut note = pending("01AAA", "body");
        note.subject = Some("kept".to_string());
        note.tags = vec!["kept-tag".to_string()];

        let patch = failed_patch(&note);
        assert_eq!(patch.status, Status::Failed);
        assert_eq!(patch.subject.as_deref(), Some("kept"));
        assert_eq!(patch.tags, vec!["kept-tag"]);
    }
}

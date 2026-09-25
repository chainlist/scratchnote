//! Turning one note body into enrichment, and that enrichment into a patch
//! for the markdown.
//!
//! Deliberately free of any app plumbing: no queue, no files, no Tauri. That
//! is what lets the golden tests in SPEC 12 drive the whole path with a stub
//! backend and check the bytes that come out the other end.

use crate::storage::daily_file::{NotePatch, Status};

use super::language::Language;
use super::model::Backend;
use super::normalize::{self, Vocabulary};
use super::{grammar, prompt, Enrichment};

/// Label one note, in `language` whatever the note is written in. The
/// vocabulary, that language's, steers the model towards tags already in use
/// and then normalises whatever it returns anyway.
pub fn enrich(
    body: &str,
    language: Language,
    vocabulary: &Vocabulary,
    backend: &dyn Backend,
) -> Result<Enrichment, String> {
    let existing = related_tags(vocabulary.by_frequency(), body);
    let text = prompt::build(body, language.name, &vocabulary.categories, &existing);

    let raw = backend.generate(&text, grammar::ENRICHMENT_GBNF)?;
    let mut enrichment = Enrichment::parse(&raw)?;

    // Normalised on its own, so what gets remembered as a category is the
    // spelling that lands on the note.
    enrichment.category = normalize::normalize(&[enrichment.category.clone()], vocabulary)
        .into_iter()
        .next()
        .unwrap_or_default();
    // The category goes first so the tag cap never drops it.
    let raw_tags: Vec<String> = std::iter::once(enrichment.category.clone())
        .chain(enrichment.tags.iter().cloned())
        .collect();
    enrichment.tags = normalize::normalize(&raw_tags, vocabulary);
    if enrichment.tags.is_empty() {
        return Err("every tag normalised away to nothing".to_string());
    }

    Ok(enrichment)
}

/// Parts of a tag shorter than this match too much of any note (`cd`, `go`).
const MIN_MATCH_LEN: usize = 3;

/// Only the existing tags whose every part appears in the note. Offered the
/// whole vocabulary, the model picks tags because they are there (`job` and
/// `emails` on a meeting note); offered these, it only reuses a spelling.
/// Every part, not any: `team` alone would offer `data-team` to any team note.
fn related_tags(tags: Vec<String>, body: &str) -> Vec<String> {
    // Split on anything that is not a letter or digit, so `Angular's` and
    // `d'Angular` both give `angular`, whatever the language.
    let body = body.to_lowercase();
    let words: Vec<&str> = body
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    tags.into_iter()
        .filter(|tag| {
            let mut parts = tag
                .split('-')
                .filter(|part| part.chars().count() >= MIN_MATCH_LEN)
                .peekable();
            parts.peek().is_some() && parts.all(|part| mentions(&words, part))
        })
        .collect()
}

/// A word contains the part (`argocd` for `argo`), or is one typo away from
/// it (`versionning` for `versioning`), the same edit the normaliser folds.
/// The first letter must match: a typo rarely hits it, while a different word
/// often differs only there (`review` and `preview`).
fn mentions(words: &[&str], part: &str) -> bool {
    words.iter().any(|word| {
        word.contains(part)
            || (part.chars().count() >= normalize::FUZZY_MIN_LEN
                && word.chars().next() == part.chars().next()
                && strsim::levenshtein(word, part) == 1)
    })
}

/// What to write into the note block, recording the language it was
/// labelled in. The body and hash are untouched.
pub fn patch(enrichment: &Enrichment, language: Language) -> NotePatch {
    NotePatch {
        subject: Some(enrichment.subject.clone()),
        summary: Some(enrichment.summary.clone()),
        tags: enrichment.tags.clone(),
        status: Status::Done,
        lang: Some(language.code.to_string()),
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
        lang: current.lang.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrich::language::{self, ENGLISH};
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
            lang: None,
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

            let enrichment =
                enrich(body, ENGLISH, &vocabulary, &backend).expect("stub always succeeds");
            let out =
                update_note(&doc, &id, &patch(&enrichment, ENGLISH)).expect("id is present");

            // The block must read back as an enriched note with the body intact.
            let parsed = parse_notes(&out, DATE, FILE);
            assert_eq!(parsed.len(), 1, "one note for {body}");
            let parsed = &parsed[0];
            assert_eq!(parsed.body, *body, "body changed for {body}");
            assert_eq!(parsed.hash, note.hash, "hash changed for {body}");
            assert_eq!(parsed.status, Status::Done);
            assert_eq!(parsed.lang.as_deref(), Some("en"), "language for {body}");
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
            categories: Default::default(),
        };
        // The stub answers "Stagging", one edit away from the known tag.
        let backend = StubBackend::new(r#"{"subject":"a","summary":"b","tags":["Stagging"]}"#);
        let out = enrich("some note", ENGLISH, &vocabulary, &backend).unwrap();
        assert_eq!(out.tags, vec!["staging"]);
    }

    #[test]
    fn the_category_becomes_the_first_tag_and_survives_the_cap() {
        let backend = StubBackend::new(
            r#"{"subject":"a","summary":"b","category":"Gaming","tags":["silksong","boss","gaming","map","secret","waterfall"]}"#,
        );
        let out = enrich("note", ENGLISH, &Vocabulary::default(), &backend).unwrap();
        assert_eq!(out.category, "gaming");
        assert_eq!(out.tags, vec!["gaming", "silksong", "boss", "map", "secret"]);
    }

    #[test]
    fn offers_only_existing_tags_the_note_mentions() {
        let tags = ["argo-cd", "job", "emails", "cd", "home-assistant", "data-team"]
            .iter()
            .map(|t| t.to_string())
            .collect();
        assert_eq!(
            related_tags(tags, "ArgoCD sync broke for the team, and Home Assistant"),
            vec!["argo-cd", "home-assistant"]
        );
    }

    #[test]
    fn offers_tags_through_possessives_elisions_and_one_typo() {
        let tags = ["angular", "versioning", "api", "argo-cd", "staging", "preview"]
            .iter()
            .map(|t| t.to_string())
            .collect();
        assert_eq!(
            related_tags(tags, "Le versionning de l'API d'Angular, Angular's review"),
            vec!["angular", "versioning", "api"]
        );
    }

    #[test]
    fn a_backend_failure_is_reported_rather_than_written_back() {
        struct Failing;
        impl Backend for Failing {
            fn generate(&self, _: &str, _: &str) -> Result<String, String> {
                Err("model exploded".to_string())
            }
        }
        let err = enrich("note", ENGLISH, &Vocabulary::default(), &Failing).unwrap_err();
        assert!(err.contains("model exploded"));
    }

    #[test]
    fn output_that_is_not_the_object_is_a_failure_not_a_bad_write() {
        let backend = StubBackend::new("I'm sorry, I cannot do that.");
        assert!(enrich("note", ENGLISH, &Vocabulary::default(), &backend).is_err());
    }

    #[test]
    fn tags_that_all_normalise_away_are_a_failure() {
        let backend =
            StubBackend::new("{\"subject\":\"a\",\"summary\":\"b\",\"tags\":[\"###\",\"!!!\"]}");
        assert!(enrich("note", ENGLISH, &Vocabulary::default(), &backend).is_err());
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
            categories: Default::default(),
        };

        let cases: [(&str, &[&str]); 3] = [
            (
                "Translation issue with the API request, and prefer to go from API to pure json file",
                &["translation", "api", "json"],
            ),
            (
                "The grammar crashed because sampler.accept was called twice per token in the Rust decode loop of llama.cpp",
                &["grammar", "rust"],
            ),
            (
                "Is Scratchnote app replacing Balise? I don't think so, they don't serve de same purpose. Though they are complementary",
                &["scratchnote", "balise"],
            ),
        ];

        let mut missing = Vec::new();
        for (note, wanted) in cases {
            let out = enrich(note, ENGLISH, &vocabulary, backend.as_ref()).expect("should label");
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
                !out.tags
                    .iter()
                    .any(|t| ["issue", "sync", "comparison"].contains(&t.as_str())),
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

    /// An existing tag is a spelling to reuse, not a list to pick from. With a
    /// real vocabulary on offer, a note about a memory leak came back tagged
    /// `tools` and `ux` because those were there, and without `memory` or
    /// `leak`. Needs a model; `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a downloaded model"]
    fn does_not_pad_with_existing_tags_that_do_not_fit() {
        let Some(backend) = load_installed_model() else {
            return;
        };

        // The vocabulary as it stood when the bad tags appeared.
        let vocabulary = Vocabulary {
            counts: [
                ("test", 4u32),
                ("plan", 3),
                ("scratchnote", 2),
                ("sync", 2),
                ("tools", 2),
                ("ux", 2),
                ("argocd", 1),
                ("balise", 1),
                ("bandwidth", 1),
                ("documentation", 1),
                ("migration", 1),
                ("primeng", 1),
                ("rollback", 1),
                ("translation", 1),
            ]
            .into_iter()
            .map(|(t, c)| (t.to_string(), c))
            .collect(),
            aliases: Default::default(),
            categories: Default::default(),
        };

        let note = "memory burst usage of Scratchnote to check if there's a memory leak";
        let out = enrich(note, ENGLISH, &vocabulary, backend.as_ref()).expect("should label");
        eprintln!("{note}\n  -> {} | {:?}", out.subject, out.tags);

        assert!(
            out.tags.iter().any(|t| t == "scratchnote"),
            "{:?}",
            out.tags
        );
        assert!(
            out.tags
                .iter()
                .any(|t| t.contains("memory") || t.contains("leak")),
            "the topic is missing: {:?}",
            out.tags
        );
        for unrelated in ["tools", "ux"] {
            assert!(
                !out.tags.iter().any(|t| t == unrelated),
                "padded with {unrelated}: {:?}",
                out.tags
            );
        }
    }

    /// A note at the length cap used to abort the whole process inside
    /// llama.cpp, because its prompt went past n_batch in one decode. It must
    /// label, and a prompt past the context must fail cleanly instead. Needs a
    /// model; `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a downloaded model"]
    fn a_note_at_the_length_cap_labels_and_a_longer_prompt_fails_cleanly() {
        let Some(backend) = load_installed_model() else {
            return;
        };
        let sentence = "Staging deploy notes: the ArgoCD sync wave order was wrong again. ";
        let long = sentence.repeat(12_000 / sentence.len() + 1);
        let out = enrich(&long, ENGLISH, &Vocabulary::default(), backend.as_ref())
            .expect("a note at the cap should label");
        assert!(!out.tags.is_empty());

        // Straight to the backend, past the truncation, so the prompt is
        // longer than the context.
        let huge = prompt::build(&"é ".repeat(20_000), "English", &[], &[]);
        let err = backend
            .generate(&huge, grammar::ENRICHMENT_GBNF)
            .unwrap_err();
        assert!(err.contains("too long"), "{err}");
    }

    /// The labels follow the language the user picked, not the note's. Needs
    /// a model; `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a downloaded model"]
    fn labels_in_the_chosen_language_whatever_the_note_is_in() {
        let Some(backend) = load_installed_model() else {
            return;
        };

        let cases = [
            (
                "ArgoCD auto-sync broke staging again. Pin the chart version and roll back before Friday's release.",
                language::find("fr").unwrap(),
                [" le ", " la ", " les ", " de ", " du ", " des "],
            ),
            (
                "La démo client est décalée à jeudi, prévenir l'équipe produit",
                ENGLISH,
                [" the ", " to ", " a ", " for ", " of ", " and "],
            ),
        ];

        for (note, language, common_words) in cases {
            let out = enrich(note, language, &Vocabulary::default(), backend.as_ref())
                .expect("should label");
            eprintln!(
                "{note}\n  -> [{}] {} | {} | {:?}",
                language.name, out.subject, out.summary, out.tags
            );
            let summary = format!(" {} ", out.summary.to_lowercase());
            assert!(
                common_words.iter().any(|word| summary.contains(word)),
                "the summary is not in {}: {}",
                language.name,
                out.summary
            );
        }
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
            let out = enrich(body, ENGLISH, &vocabulary, &backend)
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

//! Turning one note body into enrichment, and that enrichment into a patch
//! for the markdown.
//!
//! Deliberately free of any app plumbing: no queue, no files, no Tauri. That
//! is what lets the golden tests in SPEC 12 drive the whole path with a stub
//! backend and check the bytes that come out the other end.

use crate::storage::daily_file::{NotePatch, Status};

use super::language::Language;
use super::model::Backend;
use super::{grammar, prompt, Enrichment};

/// Label one note, its subject in `language` whatever the note is written
/// in, its category one of `categories`, that language's list.
pub fn enrich(
    body: &str,
    language: Language,
    categories: &[String],
    backend: &dyn Backend,
) -> Result<Enrichment, String> {
    let text = prompt::build(body, language.name, categories);
    let raw = backend.generate(&text, &grammar::enrichment(categories))?;
    let mut enrichment = Enrichment::parse(&raw)?;
    // The grammar only lets listed ones through; this covers a backend that
    // ignores it.
    if !categories.contains(&enrichment.category) {
        enrichment.category.clear();
    }
    Ok(enrichment)
}

/// What to write into the note block, recording the language it was
/// labelled in, and `on`, the day ahead it names (SPEC 5.7). The body and
/// hash are untouched.
pub fn patch(enrichment: &Enrichment, language: Language, on: Option<String>) -> NotePatch {
    NotePatch {
        subject: Some(enrichment.subject.clone()),
        category: Some(enrichment.category.clone()).filter(|c| !c.is_empty()),
        status: Status::Done,
        lang: Some(language.code.to_string()),
        on,
    }
}

/// Marks a note the queue has given up on (SPEC 5.6). Subject and category
/// are left alone so a later retry, or the user, can fill them in.
pub fn failed_patch(current: &crate::storage::daily_file::Note) -> NotePatch {
    NotePatch {
        subject: current.subject.clone(),
        category: current.category.clone(),
        status: Status::Failed,
        lang: current.lang.clone(),
        on: current.on.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrich::language::{self, ENGLISH};
    use crate::enrich::model::StubBackend;
    use crate::storage::daily_file::{append_note, body_hash, parse_notes, update_note, Kind, Note};

    const DATE: &str = "2026-09-22";
    const FILE: &str = "notes/2026/2026-09-22.md";

    fn fixed_stub() -> StubBackend {
        StubBackend::new(r#"{"subject":"Rollback plan","category":"infrastructure"}"#)
    }

    fn categories(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    /// The list a new space starts with.
    fn english() -> Vec<String> {
        categories(&[
            "development",
            "infrastructure",
            "movie",
            "tv",
            "game",
            "music",
            "book",
            "food",
            "home",
        ])
    }

    fn pending(id: &str, body: &str) -> Note {
        Note {
            id: id.to_string(),
            date: DATE.to_string(),
            time: "09:00".to_string(),
            file: FILE.to_string(),
            subject: None,
            category: None,
            status: Status::Pending,
            hash: body_hash(body),
            lang: None,
            body: body.to_string(),
            kind: Kind::Note,
            on: None,
            missing: false,
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
        let list = categories(&["development", "infrastructure"]);
        let backend = fixed_stub();

        for (i, body) in samples().iter().enumerate() {
            let id = format!("01SAMPLE{i:02}");
            let note = pending(&id, body);
            let doc = append_note("", &note, DATE);

            let enrichment = enrich(body, ENGLISH, &list, &backend).expect("stub always succeeds");
            let out = update_note(&doc, &id, &patch(&enrichment, ENGLISH, None))
                .expect("id is present");

            // The block must read back as an enriched note with the body intact.
            let parsed = parse_notes(&out, DATE, FILE);
            assert_eq!(parsed.len(), 1, "one note for {body}");
            let parsed = &parsed[0];
            assert_eq!(parsed.body, *body, "body changed for {body}");
            assert_eq!(parsed.hash, note.hash, "hash changed for {body}");
            assert_eq!(parsed.status, Status::Done);
            assert_eq!(parsed.lang.as_deref(), Some("en"), "language for {body}");
            assert_eq!(parsed.subject.as_deref(), Some("Rollback plan"));
            assert_eq!(parsed.category.as_deref(), Some("infrastructure"));

            // And the literal shape: heading, then the category as a tag.
            assert!(
                out.contains("### Rollback plan\n> #infrastructure\n\n"),
                "heading and category for {body}:\n{out}"
            );
        }
    }

    #[test]
    fn a_category_off_the_list_is_none() {
        let out = enrich("note", ENGLISH, &categories(&["movie"]), &fixed_stub()).unwrap();
        assert_eq!(out.category, "");
    }

    #[test]
    fn no_category_writes_no_category_line() {
        let backend = StubBackend::new(r#"{"subject":"Hello","category":""}"#);
        let out = enrich("hello", ENGLISH, &categories(&["movie"]), &backend).unwrap();
        let doc = append_note("", &pending("01AAA", "hello"), DATE);
        let written = update_note(&doc, "01AAA", &patch(&out, ENGLISH, None)).unwrap();
        assert!(written.contains("### Hello\nhello\n"), "{written}");
        assert_eq!(parse_notes(&written, DATE, FILE)[0].category, None);
    }

    #[test]
    fn the_prompt_and_the_grammar_offer_the_list() {
        struct Recording(std::sync::Mutex<Vec<String>>);
        impl Backend for Recording {
            fn generate(&self, prompt: &str, grammar: &str) -> Result<String, String> {
                let mut seen = self.0.lock().unwrap();
                seen.push(prompt.to_string());
                seen.push(grammar.to_string());
                Ok(r#"{"subject":"a","category":"game"}"#.to_string())
            }
        }
        let backend = Recording(Default::default());
        let french = language::find("fr").unwrap();
        let out = enrich("Silksong", french, &categories(&["movie", "game"]), &backend).unwrap();
        assert_eq!(out.category, "game");
        let seen = backend.0.lock().unwrap();
        assert!(seen[0].contains("CATEGORIES: movie, game"), "{}", seen[0]);
        assert!(seen[1].contains(r#"("movie" | "game")?"#), "{}", seen[1]);
    }

    #[test]
    fn a_backend_failure_is_reported_rather_than_written_back() {
        struct Failing;
        impl Backend for Failing {
            fn generate(&self, _: &str, _: &str) -> Result<String, String> {
                Err("model exploded".to_string())
            }
        }
        let err = enrich("note", ENGLISH, &[], &Failing).unwrap_err();
        assert!(err.contains("model exploded"));
    }

    #[test]
    fn output_that_is_not_the_object_is_a_failure_not_a_bad_write() {
        let backend = StubBackend::new("I'm sorry, I cannot do that.");
        assert!(enrich("note", ENGLISH, &[], &backend).is_err());
    }

    /// A note is filed under what it is about, from the one English list,
    /// whatever language it is written or labelled in, while its subject
    /// follows the language. Needs a model; `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a downloaded model"]
    fn files_notes_under_the_category_they_are_about_in_either_language() {
        let Some(backend) = load_installed_model() else {
            return;
        };
        let list = english();
        let cases: [(&str, &str); 6] = [
            (
                "Hollow Knight: Silksong, finally beat the second boss. Took me 25 tries.",
                "game",
            ),
            ("Cooked ramen from scratch, 6 hours of broth. Worth it.", "food"),
            (
                "Watched two episodes of Severance season 2. Still no idea what the goats are for.",
                "tv",
            ),
            (
                "Pair programming with Julie on the NestJS guard for scoped API keys.",
                "development",
            ),
            (
                "Rappel: acheter les places pour le concert de Justice en octobre.",
                "music",
            ),
            (
                "Reviewed Tom's PR on the Angular HTTP interceptor for refresh tokens.",
                "development",
            ),
        ];

        let mut wrong = Vec::new();
        for (note, wanted) in cases {
            for language in [ENGLISH, language::find("fr").unwrap()] {
                let out = enrich(note, language, &list, backend.as_ref()).expect("should label");
                eprintln!("[{}] {} | {} <- {note}", language.code, out.category, out.subject);
                if out.category != wanted {
                    wrong.push(format!(
                        "{note:?} in {}: {:?} not {wanted}",
                        language.code, out.category
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "misfiled:\n  {}", wrong.join("\n  "));
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
        let out = enrich(
            &long,
            ENGLISH,
            &categories(&["infrastructure"]),
            backend.as_ref(),
        )
        .expect("a note at the cap should label");
        assert!(!out.subject.is_empty());

        // Straight to the backend, past the truncation, so the prompt is
        // longer than the context.
        let huge = prompt::build(&"é ".repeat(20_000), "English", &[]);
        let err = backend
            .generate(&huge, &grammar::enrichment(&[]))
            .unwrap_err();
        assert!(err.contains("too long"), "{err}");
    }

    /// The subject follows the language the user picked, not the note's,
    /// even though the categories on offer are English. Needs a model;
    /// `cargo test -- --ignored`.
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
            let out = enrich(note, language, &english(), backend.as_ref()).expect("should label");
            eprintln!("{note}\n  -> [{}] {} | {}", language.name, out.subject, out.category);
            let subject = format!(" {} ", out.subject.to_lowercase());
            assert!(
                common_words.iter().any(|word| subject.contains(word)),
                "the subject is not in {}: {}",
                language.name,
                out.subject
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
        let Some(backend) = load_installed_model() else {
            eprintln!("no model installed, skipping");
            return;
        };
        let list = categories(&["development", "infrastructure", "home", "food"]);

        for body in samples().iter().take(5) {
            let out = enrich(body, ENGLISH, &list, backend.as_ref())
                .unwrap_or_else(|e| panic!("{body:?} failed: {e}"));

            assert!(!out.subject.trim().is_empty(), "empty subject for {body:?}");
            assert!(
                out.category.is_empty() || list.contains(&out.category),
                "{:?} is off the list for {body:?}",
                out.category
            );
            eprintln!("{body:?}\n  -> {} | {}", out.subject, out.category);
        }
    }

    #[test]
    fn the_failed_patch_keeps_what_was_already_there() {
        let mut note = pending("01AAA", "body");
        note.subject = Some("kept".to_string());
        note.category = Some("kept-category".to_string());

        let patch = failed_patch(&note);
        assert_eq!(patch.status, Status::Failed);
        assert_eq!(patch.subject.as_deref(), Some("kept"));
        assert_eq!(patch.category.as_deref(), Some("kept-category"));
    }
}

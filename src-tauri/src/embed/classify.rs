//! What a note is about, read from its vector (SPEC 3.14): a part of life,
//! and for work, the kind of job. Two linear classifiers trained offline on
//! labelled notes, `classifier.json`: for each label, its weights against
//! the vector plus its bias give a score, and softmax turns the scores into
//! shares. They hold only for vectors of the embedding recipe they were
//! trained on.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::vectors::{dot, Vectors};

/// The weights, trained offline (see SPEC 3.14 for how).
const MODEL: &str = include_str!("classifier.json");
/// The part of life whose notes also get a job family.
const WORK: &str = "work";

/// One classifier: a row of weights and a bias per label.
#[derive(Deserialize)]
struct Head {
    labels: Vec<String>,
    weights: Vec<Vec<f32>>,
    bias: Vec<f32>,
}

impl Head {
    /// The label scoring best, and its share of all of them.
    fn best(&self, vector: &[f32]) -> Guess {
        let scores: Vec<f32> = self
            .weights
            .iter()
            .zip(&self.bias)
            .map(|(weights, bias)| dot(weights, vector) + bias)
            .collect();
        let (at, top) = scores
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        let sum: f32 = scores.iter().map(|score| (score - top).exp()).sum();
        Guess {
            label: self.labels[at].clone(),
            score: 1.0 / sum,
        }
    }
}

#[derive(Deserialize)]
pub struct Classifier {
    /// The embedding recipe its vectors came from, as `Vectors` names it.
    embedding: String,
    life: Head,
    job: Head,
}

/// A label and how sure of it the classifier is, from 0 to 1.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Guess {
    pub label: String,
    pub score: f32,
}

/// What a note is about: its part of life, and for work, its job family.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteLabel {
    pub life: Guess,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<Guess>,
}

/// Read once, on first use.
pub fn classifier() -> &'static Classifier {
    static CLASSIFIER: OnceLock<Classifier> = OnceLock::new();
    CLASSIFIER.get_or_init(|| serde_json::from_str(MODEL).expect("classifier.json is valid"))
}

impl Classifier {
    fn dims(&self) -> usize {
        self.life.weights.first().map_or(0, Vec::len)
    }

    pub fn reads(&self, vectors: &Vectors) -> bool {
        vectors.is_from(&self.embedding, self.dims())
    }

    pub fn label(&self, vector: &[f32]) -> NoteLabel {
        let life = self.life.best(vector);
        let job = (life.label == WORK).then(|| self.job.best(vector));
        NoteLabel { life, job }
    }
}

/// Every note's label, from `kept` while the note's text is the one it was
/// labelled from, so a call after a few notes changed costs those few.
/// `kept` loses the notes gone. None for vectors of another embedding recipe.
pub fn label_notes(
    vectors: &Vectors,
    kept: &mut HashMap<String, (String, NoteLabel)>,
) -> HashMap<String, NoteLabel> {
    let classifier = classifier();
    if !classifier.reads(vectors) {
        kept.clear();
        return HashMap::new();
    }
    kept.retain(|id, _| vectors.get(id).is_some());
    vectors
        .iter()
        .map(|(id, hash, vector)| {
            let label = match kept.get(id) {
                Some((labelled, label)) if labelled == hash => label.clone(),
                _ => {
                    let label = classifier.label(vector);
                    kept.insert(id.to_string(), (hash.to_string(), label.clone()));
                    label
                }
            };
            (id.to_string(), label)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_model_matches_the_embedding_recipe() {
        let classifier = classifier();
        assert_eq!(classifier.dims(), 768);
        assert_eq!(
            classifier.embedding,
            super::super::llama::model_id("embeddinggemma-300M-Q8_0", 768)
        );
        assert!(classifier.life.labels.iter().any(|label| label == WORK));
        for head in [&classifier.life, &classifier.job] {
            assert_eq!(head.weights.len(), head.labels.len());
            assert_eq!(head.bias.len(), head.labels.len());
            assert!(head.weights.iter().all(|row| row.len() == 768));
        }
    }

    fn head(rows: &[(&str, [f32; 2], f32)]) -> Head {
        Head {
            labels: rows.iter().map(|(label, _, _)| label.to_string()).collect(),
            weights: rows
                .iter()
                .map(|(_, weights, _)| weights.to_vec())
                .collect(),
            bias: rows.iter().map(|(_, _, bias)| *bias).collect(),
        }
    }

    #[test]
    fn the_best_label_wins_with_its_softmax_share() {
        let head = head(&[("a", [1.0, 0.0], 0.0), ("b", [0.0, 1.0], 0.0)]);
        let guess = head.best(&[2.0, 0.0]);
        assert_eq!(guess.label, "a");
        let expected = 2f32.exp() / (2f32.exp() + 1.0);
        assert!((guess.score - expected).abs() < 1e-6);
    }

    #[test]
    fn only_work_gets_a_job_family() {
        let classifier = Classifier {
            embedding: "test/2".to_string(),
            life: head(&[(WORK, [1.0, 0.0], 0.0), ("home", [0.0, 1.0], 0.0)]),
            job: head(&[("software", [1.0, 0.0], 0.0), ("retail", [0.0, 1.0], 0.0)]),
        };
        let work = classifier.label(&[1.0, 0.0]);
        assert_eq!(work.life.label, WORK);
        assert_eq!(work.job.map(|job| job.label).as_deref(), Some("software"));
        assert_eq!(classifier.label(&[0.0, 1.0]).job, None);
    }
}

//! EmbeddingGemma-300M through llama.cpp, in the same process and on the
//! same backend as the chat model.
//!
//! The model averages over every token and is trained on inputs that start
//! and end with its special tokens, so a cut text keeps its end-of-text token.

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Mutex;

use llama_cpp_2::context::params::{LlamaContextParams, LlamaPoolingType};
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::token::LlamaToken;

use super::{normalize, Embedder};
use crate::enrich::llama::{llama_backend, load_model};

/// The longest input, in tokens, and so the context and the batch too: one
/// batch above n_batch aborts the process instead of returning an error, so
/// a note is cut to fit before it is decoded. A quick note is far shorter; a
/// longer one is embedded from its first 1023 tokens.
const MAX_TOKENS: u32 = 1024;

/// How the token outputs become one vector: their average, as EmbeddingGemma
/// is trained.
const POOLING: LlamaPoolingType = LlamaPoolingType::Mean;

pub struct LlamaEmbedder {
    /// Made on first use. Contexts are not re-entrant, and a chat's question
    /// can arrive while notes are being embedded in the background, hence
    /// the lock. Declared first so it is dropped before the model it borrows.
    context: Mutex<Option<Context>>,
    /// Boxed so the context's reference to it survives `Self` moving.
    model: Box<LlamaModel>,
    use_gpu: bool,
    id: String,
    dims: usize,
}

struct Context(LlamaContext<'static>);

// SAFETY: the context is only used under the `context` lock, so by one thread
// at a time, and llama.cpp does not tie a context to the thread that made it.
unsafe impl Send for Context {}

impl LlamaEmbedder {
    /// `use_gpu` as for the chat model: false keeps everything on the CPU.
    pub fn load_with(path: &Path, use_gpu: bool) -> Result<Self, String> {
        let model = load_model(path, use_gpu)?;
        // The width llama.cpp reads pooled vectors out at.
        let dims = model.n_embd_out() as usize;
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        Ok(Self {
            context: Mutex::new(None),
            model: Box::new(model),
            use_gpu,
            id: model_id(&stem, dims),
            dims,
        })
    }

    fn new_context(&self) -> Result<LlamaContext<'static>, String> {
        let params = LlamaContextParams::default()
            .with_embeddings(true)
            .with_pooling_type(POOLING)
            .with_n_ctx(NonZeroU32::new(MAX_TOKENS))
            .with_n_batch(MAX_TOKENS)
            .with_n_ubatch(MAX_TOKENS)
            .with_op_offload(self.use_gpu);
        // SAFETY: the model is boxed and never replaced, so the reference
        // stays valid however `self` moves, and `context` is dropped before
        // `model`, so no context outlives it.
        let model: &'static LlamaModel = unsafe { &*(&*self.model as *const LlamaModel) };
        model
            .new_context(llama_backend()?, params)
            .map_err(|e| format!("could not create an embedding context: {e}"))
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        // `Always` asks for the model's special tokens, which for this one are
        // a start token before the text and an end-of-text token after it.
        let tokens = self
            .model
            .str_to_token(text, AddBos::Always)
            .map_err(|e| format!("could not tokenise the text: {e}"))?;
        let tokens = fit(tokens, MAX_TOKENS as usize, self.model.token_eos());

        let mut slot = self
            .context
            .lock()
            .map_err(|_| "the embedding model lock is poisoned".to_string())?;
        let context = match &mut *slot {
            Some(context) => context,
            None => slot.insert(Context(self.new_context()?)),
        };
        let context = &mut context.0;

        // Each text is embedded on its own, from position 0.
        context.clear_kv_cache();
        let mut batch = LlamaBatch::new(tokens.len(), 1);
        batch
            .add_sequence(&tokens, 0, true)
            .map_err(|e| format!("could not build the batch: {e}"))?;
        context
            .decode(&mut batch)
            .map_err(|e| format!("could not embed the text: {e}"))?;

        // llama.cpp hands back the raw pooled vector.
        let mut vector = context
            .embeddings_seq_ith(0)
            .map_err(|e| format!("could not read the embedding: {e}"))?
            .to_vec();
        normalize(&mut vector);
        Ok(vector)
    }
}

/// `tokens` cut to at most `limit`, ending with `eos` either way, as every
/// input the model was trained on does.
fn fit(mut tokens: Vec<LlamaToken>, limit: usize, eos: LlamaToken) -> Vec<LlamaToken> {
    if tokens.last() != Some(&eos) {
        tokens.push(eos);
    }
    if tokens.len() > limit {
        tokens.truncate(limit - 1);
        tokens.push(eos);
    }
    tokens
}

/// A question, framed as EmbeddingGemma's model card has it for retrieval.
fn query_prompt(query: &str) -> String {
    format!("task: search result | query: {query}")
}

/// A note, framed as the model card has it for the documents searched. Notes
/// have no title of their own: the subject is the model's work, and would tie
/// the vector to the labels.
fn document_prompt(text: &str) -> String {
    format!("title: none | text: {text}")
}

/// What `vectors.bin` records its vectors as: the model, their size, and how
/// a note is turned into one. Vectors made another way then load as none and
/// every note is embedded again. Named after the file alone, notes embedded
/// at the last token for Qwen3-Embedding were kept and compared with queries
/// averaged for EmbeddingGemma, which ranked notes close to at random.
fn model_id(stem: &str, dims: usize) -> String {
    format!(
        "{stem}/{dims}/{POOLING:?}/{}",
        document_prompt("").trim_end()
    )
}

impl Embedder for LlamaEmbedder {
    fn model_id(&self) -> &str {
        &self.id
    }

    fn dims(&self) -> usize {
        self.dims
    }

    fn embed_document(&self, text: &str) -> Result<Vec<f32>, String> {
        self.embed(&document_prompt(text))
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        self.embed(&query_prompt(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EOS: LlamaToken = LlamaToken(151643);

    fn tokens(ids: &[i32]) -> Vec<LlamaToken> {
        ids.iter().map(|&id| LlamaToken(id)).collect()
    }

    #[test]
    fn a_short_text_that_ends_with_eos_is_left_alone() {
        let text = tokens(&[1, 2, 151643]);
        assert_eq!(fit(text.clone(), 8, EOS), text);
    }

    #[test]
    fn a_text_without_eos_gets_one() {
        assert_eq!(fit(tokens(&[1, 2]), 8, EOS), tokens(&[1, 2, 151643]));
        assert_eq!(fit(Vec::new(), 8, EOS), tokens(&[151643]));
    }

    #[test]
    fn a_long_text_is_cut_to_the_limit_and_still_ends_with_eos() {
        assert_eq!(
            fit(tokens(&[1, 2, 3, 4, 5, 151643]), 4, EOS),
            tokens(&[1, 2, 3, 151643])
        );
        assert_eq!(
            fit(tokens(&[1, 2, 3, 4, 5]), 4, EOS),
            tokens(&[1, 2, 3, 151643])
        );
        assert_eq!(fit(tokens(&[1, 2, 3, 151643]), 4, EOS).len(), 4);
    }

    #[test]
    fn queries_and_notes_are_framed_as_the_model_was_trained() {
        assert_eq!(
            query_prompt("where did I park?"),
            "task: search result | query: where did I park?"
        );
        assert_eq!(
            document_prompt("Level -2, spot 41"),
            "title: none | text: Level -2, spot 41"
        );
    }

    #[test]
    fn the_vectors_are_named_after_how_notes_are_embedded() {
        assert_eq!(
            model_id("embeddinggemma-300M-Q8_0", 768),
            "embeddinggemma-300M-Q8_0/768/Mean/title: none | text:"
        );
    }

    /// Needs the embedding model in `~/Scratchnote/models`;
    /// `cargo test ranks_the_note -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the downloaded embedding model"]
    fn ranks_the_note_that_answers_first_in_either_language() {
        use crate::enrich::model::{model_file, EmbeddingModel};

        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
        let root = std::path::PathBuf::from(home.unwrap()).join("Scratchnote");
        let path = model_file(&root, EmbeddingModel);
        if !path.is_file() {
            eprintln!("no embedding model at {}, skipping", path.display());
            return;
        }
        let embedder = LlamaEmbedder::load_with(&path, true).expect("the model should load");
        assert_eq!(
            embedder.model_id(),
            "embeddinggemma-300M-Q8_0/768/Mean/title: none | text:"
        );
        assert_eq!(embedder.dims(), 768);

        let notes = [
            "Rendez-vous chez le dentiste mardi à 14h pour le détartrage",
            "Pasta carbonara: guanciale, pecorino, egg yolks and black pepper, no cream",
            "The staging deploy failed because the Argo CD sync timed out",
        ];
        let vectors: Vec<Vec<f32>> = notes
            .iter()
            .map(|note| embedder.embed_document(note).expect("should embed"))
            .collect();
        for vector in &vectors {
            assert_eq!(vector.len(), 768);
            let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-3, "not unit length: {norm}");
        }

        // The first two ask in the other language than the note they want.
        let questions = [
            ("When is my dentist appointment?", 0),
            ("Quelle est ma recette de pâtes ?", 1),
            ("Why did the deployment break?", 2),
        ];
        for (question, wanted) in questions {
            let query = embedder.embed_query(question).expect("should embed");
            let scores: Vec<f32> = vectors
                .iter()
                .map(|v| v.iter().zip(&query).map(|(a, b)| a * b).sum())
                .collect();
            eprintln!("{question}");
            for (note, score) in notes.iter().zip(&scores) {
                eprintln!("  {score:.3}  {note}");
            }
            let best = (0..scores.len())
                .max_by(|&a, &b| scores[a].total_cmp(&scores[b]))
                .unwrap();
            assert_eq!(best, wanted, "{question:?} ranked {:?} first", notes[best]);
        }
    }
}

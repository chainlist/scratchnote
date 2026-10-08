//! EmbeddingGemma-300M through llama.cpp, embedded in the app and always on
//! the CPU.
//!
//! The model averages over every token and is trained on inputs that start
//! and end with its special tokens, so a cut text keeps its end-of-text token.

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use llama_cpp_2::context::params::{LlamaContextParams, LlamaPoolingType};
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::token::LlamaToken;

use super::{normalize, Embedder};

/// The longest input, in tokens, and so the context and the batch too: one
/// batch above n_batch aborts the process instead of returning an error, so
/// a note is cut to fit before it is decoded. A quick note is far shorter; a
/// longer one is embedded from its first 1023 tokens.
const MAX_TOKENS: u32 = 1024;

/// How the token outputs become one vector: their average, as EmbeddingGemma
/// is trained.
const POOLING: LlamaPoolingType = LlamaPoolingType::Mean;

/// llama.cpp allows one initialised backend per process, so it is made once
/// and never dropped.
static BACKEND: OnceLock<Result<LlamaBackend, String>> = OnceLock::new();

/// The process's llama.cpp backend, started on first use.
fn llama_backend() -> Result<&'static LlamaBackend, String> {
    BACKEND
        .get_or_init(|| LlamaBackend::init().map_err(|e| format!("llama.cpp would not start: {e}")))
        .as_ref()
        .map_err(Clone::clone)
}

/// Loads a GGUF on the CPU. No devices at all gets llama.cpp's plain CPU
/// path, with its repacked CPU kernels.
fn load_model(path: &Path) -> Result<LlamaModel, String> {
    let backend = llama_backend()?;
    let params = LlamaModelParams::default()
        .with_n_gpu_layers(0)
        .with_devices(&[])
        .map_err(|e| format!("could not keep the model off the GPU: {e}"))?;
    LlamaModel::load_from_file(backend, path, &params)
        .map_err(|e| format!("could not load {}: {e}", path.display()))
}

pub struct LlamaEmbedder {
    /// Made on first use. Contexts are not re-entrant, and a search can
    /// arrive while notes are being embedded in the background, hence the
    /// lock. Declared first so it is dropped before the model it borrows.
    context: Mutex<Option<Context>>,
    /// Boxed so the context's reference to it survives `Self` moving.
    model: Box<LlamaModel>,
    id: String,
    dims: usize,
}

struct Context(LlamaContext<'static>);

// SAFETY: the context is only used under the `context` lock, so by one thread
// at a time, and llama.cpp does not tie a context to the thread that made it.
unsafe impl Send for Context {}

impl LlamaEmbedder {
    /// On the CPU. The model stays loaded, and on an integrated GPU, whose
    /// memory is the system's, llama.cpp holds a copy of the weights there
    /// and a second copy of the vocabulary on the host, none of it backed by
    /// the file: about 900 MB against 260 MB on the CPU, where the file is
    /// mapped and only the pages read count. A note takes 31 ms there against
    /// 13 ms on the GPU, quick enough for a search or a draft.
    pub fn load(path: &Path) -> Result<Self, String> {
        let model = load_model(path)?;
        // The width llama.cpp reads pooled vectors out at.
        let dims = model.n_embd_out() as usize;
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        Ok(Self {
            context: Mutex::new(None),
            model: Box::new(model),
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
            .with_op_offload(false);
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
/// have no title of their own.
fn document_prompt(text: &str) -> String {
    format!("title: none | text: {}", spell_tasks(text))
}

/// The text with each task's box spelled out, `- [ ] milk` as `To do: milk`
/// and `- [x] eggs` as `Done: eggs`: to the model a box is only punctuation,
/// while the words bring the note closer to a question about what is left to
/// do, asked in English or in another language.
fn spell_tasks(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let item = line.trim_start_matches([' ', '\t']);
            let indent = &line[..line.len() - item.len()];
            let word = match item.get(..5) {
                Some("- [ ]") => "To do:",
                Some("- [x]" | "- [X]") => "Done:",
                _ => return line.to_string(),
            };
            // As in markdown, a box is only a task's when a space or the end
            // of the line follows it.
            let rest = &item[5..];
            if rest.is_empty() || rest.starts_with([' ', '\t', '\r', '\n']) {
                format!("{indent}{word}{rest}")
            } else {
                line.to_string()
            }
        })
        .collect()
}

/// What `space.db` records its vectors as: the model, their size, and how
/// a note is turned into one, its framing and the words its tasks become.
/// Vectors made another way then load as none and every note is embedded
/// again. Named after the file alone, notes embedded at the last token for
/// Qwen3-Embedding were kept and compared with queries averaged for
/// EmbeddingGemma, which ranked notes close to at random.
pub(super) fn model_id(stem: &str, dims: usize) -> String {
    format!(
        "{stem}/{dims}/{POOLING:?}/{}/{}|{}",
        document_prompt("").trim_end(),
        spell_tasks("- [ ]"),
        spell_tasks("- [x]")
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
    fn task_boxes_are_spelled_out_as_words() {
        assert_eq!(
            spell_tasks("Shopping\n- [ ] milk\n  - [x] eggs\r\n- [X] bread\n- [ ]"),
            "Shopping\nTo do: milk\n  Done: eggs\r\nDone: bread\nTo do:"
        );
        // Not a task: no space after the box, no bullet, or not at the start.
        assert_eq!(spell_tasks("- [x]y"), "- [x]y");
        assert_eq!(spell_tasks("[ ] milk"), "[ ] milk");
        assert_eq!(spell_tasks("buy - [ ] milk"), "buy - [ ] milk");
    }

    #[test]
    fn the_vectors_are_named_after_how_notes_are_embedded() {
        assert_eq!(
            model_id("embeddinggemma-300M-Q8_0", 768),
            "embeddinggemma-300M-Q8_0/768/Mean/title: none | text:/To do:|Done:"
        );
    }

    /// Needs the embedding model the app downloaded;
    /// `cargo test ranks_the_note -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the downloaded embedding model"]
    fn ranks_the_note_that_answers_first_in_either_language() {
        use crate::embed::model::{installed_local_data, model_file, EmbeddingModel};

        let path = model_file(&installed_local_data().unwrap(), EmbeddingModel);
        if !path.is_file() {
            eprintln!("no embedding model at {}, skipping", path.display());
            return;
        }
        let embedder = LlamaEmbedder::load(&path).expect("the model should load");
        assert_eq!(
            embedder.model_id(),
            "embeddinggemma-300M-Q8_0/768/Mean/title: none | text:/To do:|Done:"
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

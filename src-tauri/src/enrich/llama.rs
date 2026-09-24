//! The real backend: llama.cpp embedded through `llama-cpp-2`, no server.
//!
//! Sampling is grammar-constrained, which is what makes the output valid JSON
//! by construction rather than by luck (SPEC 5.3).

use std::num::NonZeroU32;
use std::sync::Mutex;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::data::LlamaTokenData;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use llama_cpp_2::token::LlamaToken;

use super::model::{Backend, CONTEXT_TOKENS, MAX_OUTPUT_TOKENS, TEMPERATURE};

/// Prompt tokens evaluated per decode call. Well under llama.cpp's default
/// n_batch of 2048, and above a typical prompt, so short notes still go in
/// one call.
const PROMPT_CHUNK: usize = 512;

pub struct LlamaCpp {
    /// The context kept from one note to the next, so the start every prompt
    /// shares is only evaluated once. llama.cpp contexts are not re-entrant,
    /// and the queue runs one job at a time anyway, so one guarded context is
    /// enough. Declared first so it is dropped before the model it borrows.
    session: Mutex<Option<Session>>,
    backend: LlamaBackend,
    /// Boxed so the context's reference to it survives `Self` moving.
    model: Box<LlamaModel>,
    use_gpu: bool,
}

/// A context and the tokens its cache holds, in order.
struct Session {
    context: LlamaContext<'static>,
    cached: Vec<LlamaToken>,
}

// SAFETY: the context is only used under the `session` lock, so by one thread
// at a time, and llama.cpp does not tie a context to the thread that made it.
unsafe impl Send for Session {}

impl LlamaCpp {
    /// Loads the weights once and keeps them resident (SPEC 5.1), on the GPU
    /// when the build has a GPU backend and a device is found. The app goes
    /// through `load_with` and its setting; tests take the default.
    #[cfg(test)]
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        Self::load_with(path, true)
    }

    /// `use_gpu: false` keeps every layer and every op on the CPU, even in a
    /// build with a GPU backend.
    pub fn load_with(path: &std::path::Path, use_gpu: bool) -> Result<Self, String> {
        let backend =
            LlamaBackend::init().map_err(|e| format!("llama.cpp would not start: {e}"))?;

        let params = if use_gpu {
            LlamaModelParams::default().with_n_gpu_layers(u32::MAX)
        } else {
            // Zero layers alone is not enough: with a GPU device still attached,
            // llama.cpp stages CPU work through the GPU driver's host buffers
            // and skips its repacked CPU kernels, which made CPU mode about
            // half again slower than a CPU-only build. No devices at all gets
            // the plain CPU path.
            LlamaModelParams::default()
                .with_n_gpu_layers(0)
                .with_devices(&[])
                .map_err(|e| format!("could not keep the model off the GPU: {e}"))?
        };
        let model = LlamaModel::load_from_file(&backend, path, &params)
            .map_err(|e| format!("could not load {}: {e}", path.display()))?;

        Ok(Self {
            session: Mutex::new(None),
            backend,
            model: Box::new(model),
            use_gpu,
        })
    }

    fn new_context(&self) -> Result<LlamaContext<'static>, String> {
        // With no layers on the GPU, llama.cpp would still send large prompt
        // batches there unless op offload is off too.
        let params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(CONTEXT_TOKENS))
            .with_op_offload(self.use_gpu);
        // SAFETY: the model is boxed and never replaced, so the reference
        // stays valid however `self` moves, and `session` is dropped before
        // `model`, so no context outlives it.
        let model: &'static LlamaModel = unsafe { &*(&*self.model as *const LlamaModel) };
        model
            .new_context(&self.backend, params)
            .map_err(|e| format!("could not create a context: {e}"))
    }

    fn run(
        &self,
        slot: &mut Option<Session>,
        prompt: &str,
        grammar: &str,
    ) -> Result<String, String> {
        let session = match slot {
            Some(session) => session,
            None => slot.insert(Session {
                context: self.new_context()?,
                cached: Vec::new(),
            }),
        };
        let context = &mut session.context;

        let tokens = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| format!("could not tokenise the prompt: {e}"))?;

        // Keep what the cache already holds of this prompt, at least the
        // system prompt every note shares, and evaluate only the rest. The
        // last token is always evaluated again, for logits to sample from.
        let mut reuse = session
            .cached
            .iter()
            .zip(&tokens)
            .take_while(|(cached, token)| cached == token)
            .count()
            .min(tokens.len().saturating_sub(1));
        // Models with recurrent state cannot drop only the tail; those start
        // over.
        let trimmed = context
            .clear_kv_cache_seq(Some(0), Some(reuse as u32), None)
            .unwrap_or(false);
        if !trimmed {
            context.clear_kv_cache();
            reuse = 0;
        }
        session.cached.truncate(reuse);

        // Past the context, llama.cpp would have nowhere to put the answer.
        if tokens.len() + MAX_OUTPUT_TOKENS as usize > CONTEXT_TOKENS as usize {
            return Err(format!(
                "the note is too long to label: {} prompt tokens, the limit is {}",
                tokens.len(),
                CONTEXT_TOKENS - MAX_OUTPUT_TOKENS
            ));
        }

        // In chunks: one batch above llama.cpp's n_batch aborts the process
        // instead of returning an error, and a long note gets there.
        let last = tokens.len().saturating_sub(1);
        let mut batch = LlamaBatch::new(PROMPT_CHUNK, 1);
        for chunk in (reuse..tokens.len()).collect::<Vec<_>>().chunks(PROMPT_CHUNK) {
            batch.clear();
            for &i in chunk {
                batch
                    .add(tokens[i], i as i32, &[0], i == last)
                    .map_err(|e| format!("could not build the batch: {e}"))?;
            }
            context
                .decode(&mut batch)
                .map_err(|e| format!("could not evaluate the prompt: {e}"))?;
        }
        session.cached.extend_from_slice(&tokens[reuse..]);

        // The grammar is the whole point: the sampler cannot emit anything the
        // schema does not allow.
        let mut grammar = LlamaSampler::grammar(&self.model, grammar, "root")
            .map_err(|e| format!("the enrichment grammar is not valid: {e}"))?;

        let mut out = String::new();
        // Decoding token by token can split a multi-byte character across two
        // tokens, which matters as soon as a note is not plain ASCII.
        let mut decoder = encoding_rs::UTF_8.new_decoder();

        // The grammar only allows end-of-generation once the object is closed,
        // so running out of tokens is the one way an answer comes back short.
        let mut finished = false;
        // Where the logits of the last decoded batch sit: the final prompt
        // token, last in the final chunk, then the single token of each step.
        let mut logits_at = batch.n_tokens() - 1;
        for position in (tokens.len() as i32..).take(MAX_OUTPUT_TOKENS as usize) {
            let token = sample(context, logits_at, &mut grammar);

            if self.model.is_eog_token(token) {
                finished = true;
                break;
            }
            out.push_str(
                &self
                    .model
                    .token_to_piece(token, &mut decoder, false, None)
                    .unwrap_or_default(),
            );

            batch.clear();
            batch
                .add(token, position, &[0], true)
                .map_err(|e| format!("could not extend the batch: {e}"))?;
            logits_at = 0;
            context
                .decode(&mut batch)
                .map_err(|e| format!("could not decode: {e}"))?;
            session.cached.push(token);
        }

        if !finished {
            return Err(format!(
                "stopped at the {MAX_OUTPUT_TOKENS} token limit before the answer was complete: {}",
                super::excerpt(&out)
            ));
        }
        Ok(out)
    }
}

/// Greedy under the grammar, picking what grammar, temperature and greedy
/// chained would, without running the grammar over the whole vocabulary each
/// step, which is most of the cost of a token. As llama.cpp's common sampler
/// does: take the best token unconstrained and ask the grammar about that one
/// alone. Only when it is refused does the whole vocabulary go through the
/// grammar. The best token the grammar allows is the same either way, so the
/// output does not change.
fn sample(context: &LlamaContext, logits_at: i32, grammar: &mut LlamaSampler) -> LlamaToken {
    // Scaled as the temperature sampler would, so that two logits it rounds
    // to the same value tie here too and the first one wins, as in greedy.
    let best = |logits: &mut dyn Iterator<Item = f32>| {
        let mut best = (0, f32::NEG_INFINITY);
        for (i, logit) in logits.enumerate() {
            let logit = logit / TEMPERATURE;
            if i == 0 || logit > best.1 {
                best = (i, logit);
            }
        }
        best.0
    };

    let logits = context.get_logits_ith(logits_at);
    let first = best(&mut logits.iter().copied());
    let mut alone = LlamaTokenDataArray::new(
        vec![LlamaTokenData::new(
            LlamaToken(first as i32),
            logits[first],
            0.0,
        )],
        false,
    );
    alone.apply_sampler(grammar);
    let token = if alone.data[0].logit() != f32::NEG_INFINITY {
        LlamaToken(first as i32)
    } else {
        let mut all = context.token_data_array_ith(logits_at);
        all.apply_sampler(grammar);
        // The array is in token order, so an index is the token id.
        LlamaToken(best(&mut all.data.iter().map(|d| d.logit())) as i32)
    };

    // Applying the grammar only masks; accepting is what advances it, and it
    // must happen exactly once per token or its stacks desync and llama.cpp
    // aborts inside llama_grammar_reject_candidates.
    grammar.accept(token);
    token
}

/// The GPUs llama.cpp can use, by name. Empty when the machine has no device
/// the compiled backend supports.
pub fn gpu_devices() -> Vec<String> {
    use llama_cpp_2::LlamaBackendDeviceType::{Gpu, IntegratedGpu};
    llama_cpp_2::list_llama_ggml_backend_devices()
        .into_iter()
        .filter(|d| matches!(d.device_type, Gpu | IntegratedGpu))
        .map(|d| d.description)
        .collect()
}

impl Backend for LlamaCpp {
    fn generate(&self, prompt: &str, grammar: &str) -> Result<String, String> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| "the model lock is poisoned".to_string())?;
        let result = self.run(&mut session, prompt, grammar);
        // A run that fails part way can leave the cache out of step with what
        // the session thinks it holds, so the next note starts from a new
        // context.
        if result.is_err() {
            *session = None;
        }
        result
    }
}

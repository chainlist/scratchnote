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

use super::model::{Backend, CONTEXT_TOKENS, LONG_CONTEXT_TOKENS, MAX_OUTPUT_TOKENS, TEMPERATURE};

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
    /// The larger context a chat runs in, made on first use. Kept apart so
    /// enrichment cannot evict the index a chat left in its cache.
    long_session: Mutex<Option<Session>>,
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
            long_session: Mutex::new(None),
            backend,
            model: Box::new(model),
            use_gpu,
        })
    }

    fn new_context(&self, n_ctx: u32) -> Result<LlamaContext<'static>, String> {
        // With no layers on the GPU, llama.cpp would still send large prompt
        // batches there unless op offload is off too.
        let params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(n_ctx))
            .with_op_offload(self.use_gpu);
        // SAFETY: the model is boxed and never replaced, so the reference
        // stays valid however `self` moves, and `session` is dropped before
        // `model`, so no context outlives it.
        let model: &'static LlamaModel = unsafe { &*(&*self.model as *const LlamaModel) };
        model
            .new_context(&self.backend, params)
            .map_err(|e| format!("could not create a context: {e}"))
    }

    /// The session in `slot`, made with a context of `n_ctx` tokens if there
    /// is none yet.
    fn session<'s>(
        &self,
        slot: &'s mut Option<Session>,
        n_ctx: u32,
    ) -> Result<&'s mut Session, String> {
        Ok(match slot {
            Some(session) => session,
            None => slot.insert(Session {
                context: self.new_context(n_ctx)?,
                cached: Vec::new(),
            }),
        })
    }

    fn tokenize(&self, prompt: &str) -> Result<Vec<LlamaToken>, String> {
        self.model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| format!("could not tokenise the prompt: {e}"))
    }

    /// Bring the session's cache in line with `tokens`, evaluating only what
    /// it does not already hold, and leave room for `reserve` more. Returns
    /// where the logits of the last token sit, to sample the next one from.
    fn feed(
        &self,
        session: &mut Session,
        tokens: &[LlamaToken],
        n_ctx: u32,
        reserve: u32,
    ) -> Result<i32, String> {
        let context = &mut session.context;

        // Keep what the cache already holds of this prompt, at least the
        // system prompt every note shares, and evaluate only the rest. The
        // last token is always evaluated again, for logits to sample from.
        let mut reuse = session
            .cached
            .iter()
            .zip(tokens)
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
        if tokens.len() + reserve as usize > n_ctx as usize {
            return Err(format!(
                "the prompt is too long: {} tokens, the limit is {}",
                tokens.len(),
                n_ctx - reserve
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

        // The final prompt token, last in the final chunk.
        Ok(batch.n_tokens() - 1)
    }

    /// Evaluate one sampled token so the next can be sampled from it.
    fn step(session: &mut Session, token: LlamaToken, position: i32) -> Result<(), String> {
        let mut batch = LlamaBatch::new(1, 1);
        batch
            .add(token, position, &[0], true)
            .map_err(|e| format!("could not extend the batch: {e}"))?;
        session
            .context
            .decode(&mut batch)
            .map_err(|e| format!("could not decode: {e}"))?;
        session.cached.push(token);
        Ok(())
    }

    fn run(
        &self,
        slot: &mut Option<Session>,
        n_ctx: u32,
        prompt: &str,
        grammar: &str,
    ) -> Result<String, String> {
        let session = self.session(slot, n_ctx)?;
        let tokens = self.tokenize(prompt)?;
        // Where the logits of the last decoded batch sit: the final prompt
        // token, then the single token of each step.
        let mut logits_at = self.feed(session, &tokens, n_ctx, MAX_OUTPUT_TOKENS)?;

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
        for position in (tokens.len() as i32..).take(MAX_OUTPUT_TOKENS as usize) {
            let token = sample(&session.context, logits_at, &mut grammar);

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
            Self::step(session, token, position)?;
            logits_at = 0;
        }

        if !finished {
            return Err(format!(
                "stopped at the {MAX_OUTPUT_TOKENS} token limit before the answer was complete: {}",
                super::excerpt(&out)
            ));
        }
        Ok(out)
    }

    /// Free text, handed to `on_piece` as it comes. Sampled rather than
    /// greedy, with a light repetition penalty: greedy prose from a small
    /// model loops. Ends at the end of the answer, at `max_tokens`, or when
    /// `on_piece` returns false, and returns what was written either way.
    fn stream(
        &self,
        slot: &mut Option<Session>,
        n_ctx: u32,
        prompt: &str,
        max_tokens: u32,
        on_piece: &mut dyn FnMut(&str) -> bool,
    ) -> Result<String, String> {
        let session = self.session(slot, n_ctx)?;
        let tokens = self.tokenize(prompt)?;
        let mut logits_at = self.feed(session, &tokens, n_ctx, max_tokens)?;

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::penalties(self.model.n_vocab(), 64, 1.1, 0.0, 0.0),
            LlamaSampler::top_k(40),
            LlamaSampler::top_p(0.9, 1),
            LlamaSampler::temp(CHAT_TEMPERATURE),
            LlamaSampler::dist(seed),
        ]);

        let mut out = String::new();
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        for position in (tokens.len() as i32..).take(max_tokens as usize) {
            // Samples and accepts in one call.
            let token = sampler.sample(&session.context, logits_at);
            if self.model.is_eog_token(token) {
                break;
            }
            let piece = self
                .model
                .token_to_piece(token, &mut decoder, false, None)
                .unwrap_or_default();
            out.push_str(&piece);
            // Stopped before the token goes into the cache, which then still
            // matches what the session says it holds.
            if !on_piece(&piece) {
                break;
            }
            Self::step(session, token, position)?;
            logits_at = 0;
        }
        Ok(out)
    }
}

/// Warm enough to read as prose, cool enough to stay on the notes.
const CHAT_TEMPERATURE: f32 = 0.6;

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

impl LlamaCpp {
    fn run_in(
        &self,
        slot: &Mutex<Option<Session>>,
        n_ctx: u32,
        prompt: &str,
        grammar: &str,
    ) -> Result<String, String> {
        let mut session = slot
            .lock()
            .map_err(|_| "the model lock is poisoned".to_string())?;
        let result = self.run(&mut session, n_ctx, prompt, grammar);
        // A run that fails part way can leave the cache out of step with what
        // the session thinks it holds, so the next call starts from a new
        // context.
        if result.is_err() {
            *session = None;
        }
        result
    }
}

impl Backend for LlamaCpp {
    fn generate(&self, prompt: &str, grammar: &str) -> Result<String, String> {
        self.run_in(&self.session, CONTEXT_TOKENS, prompt, grammar)
    }

    fn stream_long(
        &self,
        prompt: &str,
        max_tokens: u32,
        on_piece: &mut dyn FnMut(&str) -> bool,
    ) -> Result<String, String> {
        let mut session = self
            .long_session
            .lock()
            .map_err(|_| "the model lock is poisoned".to_string())?;
        let result = self.stream(
            &mut session,
            LONG_CONTEXT_TOKENS,
            prompt,
            max_tokens,
            on_piece,
        );
        if result.is_err() {
            *session = None;
        }
        result
    }

    fn prefill_long(&self, prompt: &str) -> Result<(), String> {
        let mut session = self
            .long_session
            .lock()
            .map_err(|_| "the model lock is poisoned".to_string())?;
        let result = (|| {
            let tokens = self.tokenize(prompt)?;
            let session = self.session(&mut session, LONG_CONTEXT_TOKENS)?;
            self.feed(session, &tokens, LONG_CONTEXT_TOKENS, 0)
                .map(|_| ())
        })();
        if result.is_err() {
            *session = None;
        }
        result
    }
}

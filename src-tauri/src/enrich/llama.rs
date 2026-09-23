//! The real backend: llama.cpp embedded through `llama-cpp-2`, no server.
//!
//! Sampling is grammar-constrained, which is what makes the output valid JSON
//! by construction rather than by luck (SPEC 5.3).

use std::num::NonZeroU32;
use std::sync::Mutex;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use super::model::{Backend, CONTEXT_TOKENS, MAX_OUTPUT_TOKENS, TEMPERATURE};

pub struct LlamaCpp {
    backend: LlamaBackend,
    model: LlamaModel,
    use_gpu: bool,
    /// llama.cpp contexts are not re-entrant, and the queue runs one job at a
    /// time anyway, so one guarded context is enough.
    lock: Mutex<()>,
}

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
            backend,
            model,
            use_gpu,
            lock: Mutex::new(()),
        })
    }

    fn run(&self, prompt: &str, grammar: &str) -> Result<String, String> {
        // With no layers on the GPU, llama.cpp would still send large prompt
        // batches there unless op offload is off too.
        let params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(CONTEXT_TOKENS))
            .with_op_offload(self.use_gpu);
        let mut context = self
            .model
            .new_context(&self.backend, params)
            .map_err(|e| format!("could not create a context: {e}"))?;

        let tokens = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| format!("could not tokenise the prompt: {e}"))?;

        let mut batch = LlamaBatch::new(tokens.len().max(1), 1);
        let last = tokens.len().saturating_sub(1);
        for (i, token) in tokens.iter().enumerate() {
            batch
                .add(*token, i as i32, &[0], i == last)
                .map_err(|e| format!("could not build the batch: {e}"))?;
        }
        context
            .decode(&mut batch)
            .map_err(|e| format!("could not evaluate the prompt: {e}"))?;

        // The grammar is the whole point: the sampler cannot emit anything the
        // schema does not allow.
        let grammar = LlamaSampler::grammar(&self.model, grammar, "root")
            .map_err(|e| format!("the enrichment grammar is not valid: {e}"))?;
        let mut sampler = LlamaSampler::chain_simple([
            grammar,
            LlamaSampler::temp(TEMPERATURE),
            LlamaSampler::greedy(),
        ]);

        let mut out = String::new();
        // Decoding token by token can split a multi-byte character across two
        // tokens, which matters as soon as a note is not plain ASCII.
        let mut decoder = encoding_rs::UTF_8.new_decoder();

        // The grammar only allows end-of-generation once the object is closed,
        // so running out of tokens is the one way an answer comes back short.
        let mut finished = false;
        for position in (tokens.len() as i32..).take(MAX_OUTPUT_TOKENS as usize) {
            // `sample` already accepts the token internally. Accepting it
            // again here advances the grammar twice per token, which desyncs
            // its stacks and aborts inside llama_grammar_reject_candidates.
            let token = sampler.sample(&context, -1);

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
            context
                .decode(&mut batch)
                .map_err(|e| format!("could not decode: {e}"))?;
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
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "the model lock is poisoned".to_string())?;
        self.run(prompt, grammar)
    }
}

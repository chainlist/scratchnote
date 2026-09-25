//! What the onboarding reads about the machine to suggest a model, before
//! any model is on disk to time. It is an estimate: once a model is
//! installed, the settings benchmark is the real measure.

use serde::Serialize;

use super::model::Variant;

const GIB: u64 = 1 << 30;
/// A dedicated GPU with this much memory holds the default model, its
/// context and the embedding model with room to spare.
const GPU_MEMORY: u64 = 6 * GIB;
/// On the CPU the default model keeps up only on a machine this large.
const CPU_MEMORY: u64 = 16 * GIB;
const CPU_THREADS: usize = 8;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    /// In bytes; zero when the driver does not say.
    pub memory: u64,
    /// Shares the system memory rather than having its own.
    pub integrated: bool,
}

/// Why the suggested model was picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    /// A dedicated GPU with room for the default model.
    Gpu,
    /// No such GPU, but memory and cores enough to run it on the CPU.
    Cpu,
    /// Too little of either for the default model to keep up.
    Small,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    pub gpus: Vec<Gpu>,
    /// System memory in bytes; zero when unknown.
    pub memory: u64,
    pub threads: usize,
    pub recommended: Variant,
    pub reason: Reason,
}

/// Read the devices llama.cpp can see, which is also what the model will
/// run on. The CPU device reports the system memory.
pub fn probe() -> Hardware {
    use llama_cpp_2::LlamaBackendDeviceType::{Cpu, Gpu as Discrete, IntegratedGpu};

    let mut gpus = Vec::new();
    let mut memory = 0;
    for device in llama_cpp_2::list_llama_ggml_backend_devices() {
        match device.device_type {
            Discrete | IntegratedGpu => gpus.push(Gpu {
                name: device.description,
                memory: device.memory_total as u64,
                integrated: device.device_type == IntegratedGpu,
            }),
            Cpu => memory = memory.max(device.memory_total as u64),
            _ => {}
        }
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let (recommended, reason) = recommend(&gpus, memory, threads);
    Hardware {
        gpus,
        memory,
        threads,
        recommended,
        reason,
    }
}

fn recommend(gpus: &[Gpu], memory: u64, threads: usize) -> (Variant, Reason) {
    if gpus.iter().any(|g| !g.integrated && g.memory >= GPU_MEMORY) {
        (Variant::Default, Reason::Gpu)
    } else if memory >= CPU_MEMORY && threads >= CPU_THREADS {
        (Variant::Default, Reason::Cpu)
    } else {
        (Variant::Light, Reason::Small)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(memory: u64, integrated: bool) -> Gpu {
        Gpu {
            name: "test".to_string(),
            memory,
            integrated,
        }
    }

    #[test]
    fn a_dedicated_gpu_with_room_takes_the_default_model() {
        let gpus = [gpu(8 * GIB, false)];
        assert_eq!(
            recommend(&gpus, 8 * GIB, 4),
            (Variant::Default, Reason::Gpu)
        );
    }

    #[test]
    fn an_integrated_or_small_gpu_does_not_count() {
        let gpus = [gpu(8 * GIB, true), gpu(2 * GIB, false)];
        assert_eq!(
            recommend(&gpus, 8 * GIB, 4),
            (Variant::Light, Reason::Small)
        );
    }

    #[test]
    fn a_large_cpu_machine_takes_the_default_model() {
        assert_eq!(
            recommend(&[], 32 * GIB, 16),
            (Variant::Default, Reason::Cpu)
        );
        assert_eq!(recommend(&[], 32 * GIB, 4), (Variant::Light, Reason::Small));
        assert_eq!(recommend(&[], 8 * GIB, 16), (Variant::Light, Reason::Small));
    }

    #[test]
    fn unknown_memory_errs_on_the_light_side() {
        assert_eq!(recommend(&[], 0, 16), (Variant::Light, Reason::Small));
    }
}

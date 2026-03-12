#![cfg(feature = "local_llm")]

use crate::chal::{DriverStatus, ExecutionError, Grammar, InferenceDriver, SystemError};
use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;
use tracing::info;

/// --- COGNITIVE NATIVE DRIVER (LLAMA-CPP-2) ---
#[allow(dead_code)]
pub struct LlamaNativeDriver {
    n_gpu_layers: u32,
    ctx_size: u32,
}

impl LlamaNativeDriver {
    /// Inicializa una instancia del Driver sin cargar un modelo.
    pub fn new(n_gpu_layers: u32, ctx_size: u32) -> anyhow::Result<Self> {
        Ok(Self {
            n_gpu_layers,
            ctx_size,
        })
    }
}

#[async_trait]
impl InferenceDriver for LlamaNativeDriver {
    async fn generate_stream(
        &self,
        _prompt: &str,
        _grammar: Option<Grammar>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>
    {
        Err(SystemError::ModelNotFound(
            "Native driver disabled for tests".into(),
        ))
    }

    async fn get_health_status(&self) -> DriverStatus {
        DriverStatus {
            is_ready: false,
            vram_usage_bytes: 0,
            active_models: vec![],
        }
    }

    async fn load_model(&mut self, path: &str) -> Result<(), SystemError> {
        info!(model_path = %path, "Mock loading GGUF model into Native Driver...");
        Ok(())
    }
}

unsafe impl Send for LlamaNativeDriver {}
unsafe impl Sync for LlamaNativeDriver {}

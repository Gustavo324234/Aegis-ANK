pub mod drivers;
use crate::plugins::PluginManager;
use crate::scheduler::{ModelPreference, SharedPCB};
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio_stream::Stream;
use tracing::info;

/// --- SYSTEM PROMPT CONSTANTS ---
pub const SYSTEM_PROMPT_MASTER: &str = r#"
[AEGIS NEURAL KERNEL - ISA v1.0]
Eres una ALU Cognitiva (Unidad Lógica Aritmética) operando dentro del Aegis Neural Kernel.
Tu objetivo es ejecutar procesos con precisión matemática y cero lenguaje de cortesía.
No saludes. No pidas disculpas. No uses frases de relleno.

REGLAS DE EJECUCIÓN:
1. Si necesitas usar una herramienta, detén tu generación e inserta una Syscall.
2. Formato de Syscall: [SYS_CALL_PLUGIN("nombre_plugin", {"clave": "valor"})]
3. Solo puedes usar los plugins listados a continuación.
"#;

/// --- CHAL ERROR SYSTEM ---
#[derive(Error, Debug, Clone)]
pub enum SystemError {
    #[error("VRAM Exhausted: cannot load model or process prompt")]
    VramExhausted,
    #[error("Driver Offline: the inference engine {0} is not responding")]
    DriverOffline(String),
    #[error("Model Not Found: {0}")]
    ModelNotFound(String),
    #[error("Hardware Failure: {0}")]
    HardwareFailure(String),
    #[error("Decision Error: {0}")]
    DecisionError(String),
}

#[derive(Error, Debug, Clone)]
pub enum ExecutionError {
    #[error("Stream Interrupted: {0}")]
    Interrupted(String),
    #[error("Safety Violation: Content blocked by filter")]
    SafetyViolation,
    #[error("Processing Timeout")]
    Timeout,
}

/// --- SUPPORT TYPES ---
#[derive(Debug, Clone, Default)]
pub struct DriverStatus {
    pub is_ready: bool,
    pub vram_usage_bytes: u64,
    pub active_models: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Grammar {
    Gbnf(String),
    JsonSchema(serde_json::Value),
}

/// --- INFERENCE DRIVER INTERFACE ---
#[async_trait]
pub trait InferenceDriver: Send + Sync {
    async fn generate_stream(
        &self,
        prompt: &str,
        grammar: Option<Grammar>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>;

    async fn get_health_status(&self) -> DriverStatus;

    async fn load_model(&mut self, model_id: &str) -> Result<(), SystemError>;
}

/// --- COGNITIVE HAL (Hardware Abstraction Layer) ---
pub struct CognitiveHAL {
    pub drivers: HashMap<String, Box<dyn InferenceDriver>>,
    pub plugin_manager: Arc<RwLock<PluginManager>>,
    pub mcp_registry: Arc<ank_mcp::registry::McpToolRegistry>,
}

impl CognitiveHAL {
    pub fn new(plugin_manager: Arc<RwLock<PluginManager>>) -> Self {
        let mut drivers: HashMap<String, Box<dyn InferenceDriver>> = HashMap::new();

        // Auto-Register CloudDriver if environment variables are populated
        if let Some(cloud_driver) = crate::chal::drivers::CloudProxyDriver::from_env() {
            drivers.insert("cloud-driver".to_string(), Box::new(cloud_driver));
            tracing::info!("CloudProxyDriver initialized via ENV vars and registered.");
        }

        Self {
            drivers,
            plugin_manager,
            mcp_registry: Arc::new(ank_mcp::registry::McpToolRegistry::new()),
        }
    }

    pub fn update_cloud_credentials(&mut self, api_url: String, model: String, api_key: String) {
        let cloud_driver =
            crate::chal::drivers::CloudProxyDriver::new(api_url, api_key, model.clone());
        self.drivers
            .insert("cloud-driver".to_string(), Box::new(cloud_driver));
        tracing::info!(model = %model, "CloudProxyDriver credentials updated dynamically and driver re-registered in HAL.");
    }

    pub fn register_driver(&mut self, id: &str, driver: Box<dyn InferenceDriver>) {
        self.drivers.insert(id.to_string(), driver);
        info!(driver_id = %id, "Driver registered in cHAL.");
    }

    /// Lógica de enrutamiento basada en política y complejidad.
    /// AUDITORÍA: Liberamos el lock del PCB inmediatamente después de extraer datos
    /// para evitar bloqueos prolongados durante la inicialización del stream.
    pub async fn route_and_execute(
        &self,
        shared_pcb: SharedPCB,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>
    {
        // 1. Extraer datos necesarios y liberar el lock inmediatamente
        let (instruction, priority, model_pref, pid) = {
            let pcb = shared_pcb.read().await;
            (
                pcb.memory_pointers.l1_instruction.clone(),
                pcb.priority,
                pcb.model_pref,
                pcb.pid.clone(),
            )
        }; // El lock se libera aquí al salir del scope

        // 2. Lógica de selección de driver (ya no depende del lock activo del PCB)
        let driver_id = match model_pref {
            ModelPreference::LocalOnly => {
                #[cfg(not(feature = "local_llm"))]
                {
                    return Err(SystemError::HardwareFailure(
                        "Motor local no compilado. Reinicie con feature 'local_llm' o use Cloud."
                            .to_string(),
                    ));
                }
                #[cfg(feature = "local_llm")]
                {
                    info!(pid = %pid, "Policy: LOCAL_ONLY. Selecting local-driver.");
                    "local-driver"
                }
            }
            ModelPreference::CloudOnly => {
                info!(pid = %pid, "Policy: CLOUD_ONLY. Selecting cloud-driver.");
                "cloud-driver"
            }
            ModelPreference::HybridSmart => {
                // Heurística de complejidad y disponibilidad local
                let is_complex = priority > 8 || instruction.len() > 1000;
                let has_local_driver =
                    cfg!(feature = "local_llm") && self.drivers.contains_key("local-driver");

                if is_complex || !has_local_driver {
                    info!(
                        pid = %pid,
                        priority = priority,
                        has_local_driver = has_local_driver,
                        "HybridSmart: Routing to CLOUD (fallback or complex)."
                    );
                    "cloud-driver"
                } else {
                    info!(
                        pid = %pid,
                        priority = priority,
                        "HybridSmart: Low complexity and local driver available. Routing to LOCAL."
                    );
                    "local-driver"
                }
            }
        };

        let driver = self.drivers.get(driver_id).ok_or_else(|| {
            if driver_id == "cloud-driver" {
                SystemError::HardwareFailure(
                    "Driver cloud no configurado o sin credenciales.".to_string(),
                )
            } else {
                SystemError::DriverOffline(driver_id.to_string())
            }
        })?;

        // 3. Ensamblaje del "Master Prompt" (Prompt Injection)
        // Concatenamos: [Master Rules] + [Active Plugins] + [Process Instruction]
        let tool_prompt = self
            .plugin_manager
            .read()
            .await
            .get_available_tools_prompt();

        let mcp_tool_prompt = self.mcp_registry.generate_system_prompt().await;

        let final_prompt = format!(
            "{}\n{}\n{}\n\n[USER_PROCESS_INSTRUCTION]\n{}",
            SYSTEM_PROMPT_MASTER, tool_prompt, mcp_tool_prompt, instruction
        );

        // 4. Ejecutar generación
        driver.generate_stream(&final_prompt, None).await
    }
}

/// --- DUMMY DRIVER FOR TESTING ---
pub struct DummyDriver {
    pub name: String,
}

#[async_trait]
impl InferenceDriver for DummyDriver {
    async fn generate_stream(
        &self,
        _prompt: &str,
        _grammar: Option<Grammar>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>
    {
        // Simulamos un stream que envía 'OK'
        let response = format!("[{}] OK", self.name);
        let stream = tokio_stream::iter(vec![Ok(response)]);
        Ok(Box::pin(stream))
    }

    async fn get_health_status(&self) -> DriverStatus {
        DriverStatus {
            is_ready: true,
            ..Default::default()
        }
    }

    async fn load_model(&mut self, _id: &str) -> Result<(), SystemError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::PCB;
    use crate::scheduler::ModelPreference;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_hybrid_smart_routing_high_priority() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let mut hal = CognitiveHAL::new(pm);

        hal.register_driver(
            "local-driver",
            Box::new(DummyDriver {
                name: "local".to_string(),
            }),
        );
        hal.register_driver(
            "cloud-driver",
            Box::new(DummyDriver {
                name: "cloud".to_string(),
            }),
        );

        // PCB con HybridSmart y Prioridad 10 (Alta)
        let mut pcb = PCB::mock("Complex Mission", 10);
        pcb.model_pref = ModelPreference::HybridSmart;
        let shared_pcb = Arc::new(RwLock::new(pcb));

        // Debe enrutar a cloud-driver
        let stream_res = hal.route_and_execute(shared_pcb).await?;
        let tokens: Vec<_> = stream_res.collect().await;

        assert_eq!(tokens.len(), 1);
        let response = tokens[0].as_ref().map_err(|e| anyhow::anyhow!("{}", e))?;
        assert!(
            response.contains("[cloud]"),
            "Debe haber seleccionado el driver cloud por alta prioridad"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_hybrid_smart_routing_low_priority() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let mut hal = CognitiveHAL::new(pm);

        hal.register_driver(
            "local-driver",
            Box::new(DummyDriver {
                name: "local".to_string(),
            }),
        );
        hal.register_driver(
            "cloud-driver",
            Box::new(DummyDriver {
                name: "cloud".to_string(),
            }),
        );

        // PCB con HybridSmart y Prioridad 5 (Baja)
        let mut pcb = PCB::mock("Simple task", 5);
        pcb.model_pref = ModelPreference::HybridSmart;
        let shared_pcb = Arc::new(RwLock::new(pcb));

        // Debe enrutar a local-driver
        let stream_res = hal.route_and_execute(shared_pcb).await?;
        let tokens: Vec<_> = stream_res.collect().await;

        let response = tokens[0].as_ref().map_err(|e| anyhow::anyhow!("{}", e))?;
        assert!(
            response.contains("[local]"),
            "Debe haber seleccionado el driver local por baja prioridad"
        );
        Ok(())
    }
}

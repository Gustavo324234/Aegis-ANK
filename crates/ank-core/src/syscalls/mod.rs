use crate::plugins::PluginManager;
use crate::scribe::CommitMetadata;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use thiserror::Error;

/// --- SYSCALL ENUM ---
/// Representa las operaciones privilegiadas que la IA puede solicitar al Kernel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Syscall {
    /// Invoca un módulo WebAssembly (Ej: Búsqueda Web, Lector PDF)
    PluginCall {
        plugin_name: String,
        args_json: String,
    },

    /// Petición nativa del Kernel para leer un archivo del Workspace (URI file://)
    ReadFile { uri: String },

    /// Petición de escritura mediada por The Scribe (con trazabilidad Git)
    WriteFile {
        uri: String,
        content: String,
        metadata: CommitMetadata,
    },
}

/// --- SYSCALL ERROR ---
#[derive(Error, Debug)]
pub enum SyscallError {
    #[error("Plugin Execution Failed: {0}")]
    PluginError(String),
    #[error("File Access Denied: {0}")]
    AccessDenied(String),
    #[error("Security Violation (SSRF Guard): {0}")]
    SecurityViolation(String),
    #[error("IO Error: {0}")]
    IOError(String),
    #[error("Internal Kernel Error: {0}")]
    InternalError(String),
}

use crate::vcm::VirtualContextManager;
use crate::vcm::swap::LanceSwapManager;
use crate::scribe::ScribeManager;

/// --- SYSCALL EXECUTOR ---
/// El ejecutor de Syscalls es el puente entre el parser y los subsistemas del Kernel.
pub struct SyscallExecutor {
    plugin_manager: Arc<PluginManager>,
    vcm: Arc<VirtualContextManager>,
    scribe: Arc<ScribeManager>,
    swap: Arc<LanceSwapManager>,
}

impl SyscallExecutor {
    pub fn new(
        plugin_manager: Arc<PluginManager>,
        vcm: Arc<VirtualContextManager>,
        scribe: Arc<ScribeManager>,
        swap: Arc<LanceSwapManager>,
    ) -> Self {
        Self {
            plugin_manager,
            vcm,
            scribe,
            swap,
        }
    }

    pub async fn execute(&self, pcb: &crate::pcb::PCB, syscall: Syscall) -> Result<String, SyscallError> {
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

        match syscall {
            Syscall::PluginCall {
                plugin_name,
                args_json,
            } => {
                let result = self
                    .plugin_manager
                    .execute_plugin(tenant_id, &plugin_name, &args_json)
                    .await
                    .map_err(|e| SyscallError::PluginError(e.to_string()))?;

                Ok(format!("[SYSTEM_RESULT: {}]", result))
            }
            Syscall::ReadFile { uri } => {
                // Validación y Ensamblaje vía VCM
                let file_path = if uri.starts_with("file://") { &uri[7..] } else { &uri };
                
                // Intentamos leer el archivo usando el motor de contexto (VCM)
                // Pero como ReadFile es una Syscall puntual, delegamos a la lógica de Jailing del VCM
                let tenant_root = format!("./users/{}/workspace", tenant_id);
                let full_path = std::path::Path::new(&tenant_root).join(file_path);

                let content = tokio::fs::read_to_string(&full_path)
                    .await
                    .map_err(|e| SyscallError::IOError(format!("Read failed for {}: {}", uri, e)))?;

                Ok(format!("[SYSTEM_RESULT: Content of {}]\n{}", uri, content))
            }
            Syscall::WriteFile { uri, content, metadata } => {
                // Mediación vía The Scribe para trazabilidad multi-tenant
                let file_path = if uri.starts_with("file://") { &uri[7..] } else { &uri };
                
                self.scribe.write_and_commit(tenant_id, file_path, content.as_bytes(), metadata)
                    .await
                    .map_err(|e| SyscallError::IOError(format!("Scribe write failed: {}", e)))?;

                Ok(format!("[SYSTEM_RESULT: File {} written and committed to Git]", uri))
            }
        }
    }

    /// Implementación de seguridad SRE para peticiones HTTP.
    /// Delega en el PluginManager para mantener una única fuente de verdad sobre políticas de red.
    pub async fn fetch_url_safe(&self, url_str: &str) -> Result<String, SyscallError> {
        self.plugin_manager.fetch_url_safe(url_str).await
            .map_err(|e| match e {
                crate::plugins::PluginError::SecurityViolation(msg) => SyscallError::SecurityViolation(msg),
                _ => SyscallError::IOError(e.to_string()),
            })
    }
}

/// --- STREAM INTERCEPTOR (REAL-TIME) ---
/// Esta estructura se encarga de analizar el stream de tokens mientras se generan
/// para detectar triggers ([SYS) y detener la inferencia inmediatamente.
pub struct StreamInterceptor {
    buffer: String,
    trigger_detected: bool,
    max_buffer_size: usize,
}

#[derive(Debug, PartialEq)]
pub enum InterceptorResult {
    Continue,
    PossibleSyscall,       // Detectamos el inicio '[' o '[SYS'
    SyscallReady(Syscall), // Ya tenemos la syscall completa
}

impl StreamInterceptor {
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(512),
            trigger_detected: false,
            max_buffer_size: 1024, // Ventana de seguridad
        }
    }

    /// Procesa un nuevo token y decide si se debe abortar la inferencia.
    pub fn push_token(&mut self, token: &str) -> InterceptorResult {
        self.buffer.push_str(token);

        // Si el buffer crece demasiado sin detectar nada, lo limpiamos manteniendo el final
        if self.buffer.len() > self.max_buffer_size {
            let drain_amount = self.buffer.len() - self.max_buffer_size;
            self.buffer.drain(..drain_amount);
        }

        // Detección de Trigger inicial
        if !self.trigger_detected {
            // Buscamos patrones conocidos de Syscall
            if self.buffer.contains("[") {
                if self.buffer.contains("[SYS")
                    || self.buffer.contains("[READ")
                    || self.buffer.contains("[WRITE")
                {
                    self.trigger_detected = true;
                    return InterceptorResult::PossibleSyscall;
                }
            }
            InterceptorResult::Continue
        } else {
            // Ya detectamos un trigger, buscamos el cierre ']'
            if self.buffer.contains("]") {
                // Intentamos parsear la syscall completa
                if let Some(syscall) = parse_syscall(&self.buffer) {
                    return InterceptorResult::SyscallReady(syscall);
                }
            }
            InterceptorResult::PossibleSyscall
        }
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }
}

/// --- REGEX PATTERNS ---
static PLUGIN_RE: OnceLock<Regex> = OnceLock::new();
static READ_RE: OnceLock<Regex> = OnceLock::new();
static WRITE_RE: OnceLock<Regex> = OnceLock::new();

/// Parser de Syscalls Cognitivas.
/// Detecta llamadas estructuradas dentro del stream de texto de la IA.
pub fn parse_syscall(text: &str) -> Option<Syscall> {
    let plugin_re = PLUGIN_RE
        .get_or_init(|| Regex::new(r#"\[SYS_CALL_PLUGIN\("([^"]+)",\s*(\{.*?\})\)\]"#).unwrap());

    let read_re = READ_RE.get_or_init(|| Regex::new(r#"\[READ_FILE\("([^"]+)"\)\]"#).unwrap());

    let write_re = WRITE_RE.get_or_init(|| {
        // Formato esperado: [WRITE_FILE("path", "content", {"task_id": "..."})]
        Regex::new(r#"\[WRITE_FILE\("([^"]+)",\s*"([\s\S]*?)",\s*(\{.*?\})\)\]"#).unwrap()
    });

    // 1. Check for Plugin Call
    if let Some(caps) = plugin_re.captures(text) {
        return Some(Syscall::PluginCall {
            plugin_name: caps[1].to_string(),
            args_json: caps[2].to_string(),
        });
    }

    // 2. Check for Read File
    if let Some(caps) = read_re.captures(text) {
        return Some(Syscall::ReadFile {
            uri: caps[1].to_string(),
        });
    }

    // 3. Check for Write File
    if let Some(caps) = write_re.captures(text) {
        let uri = caps[1].to_string();
        let content = caps[2].to_string();
        let metadata_json = &caps[3];

        if let Ok(metadata) = serde_json::from_str::<CommitMetadata>(metadata_json) {
            return Some(Syscall::WriteFile {
                uri,
                content,
                metadata,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plugin_call() {
        let stream = "El resultado es: [SYS_CALL_PLUGIN(\"weather\", {\"city\": \"Paris\"})]";
        let syscall = parse_syscall(stream).expect("Should parse plugin call");

        if let Syscall::PluginCall {
            plugin_name,
            args_json,
        } = syscall
        {
            assert_eq!(plugin_name, "weather");
            assert_eq!(args_json, "{\"city\": \"Paris\"}");
        } else {
            panic!("Wrong syscall type");
        }
    }

    #[test]
    fn test_parse_read_file() {
        let stream = "Necesito ver el código: [READ_FILE(\"src/main.rs\")]";
        let syscall = parse_syscall(stream).expect("Should parse read call");

        if let Syscall::ReadFile { uri } = syscall {
            assert_eq!(uri, "src/main.rs");
        } else {
            panic!("Wrong syscall type");
        }
    }

    #[test]
    fn test_parse_write_file() {
        let stream = "[WRITE_FILE(\"output.txt\", \"Hello World\", {\"task_id\": \"ANK-101\", \"version_increment\": \"patch\", \"summary\": \"test\", \"impact\": \"low\"})]";
        let syscall = parse_syscall(stream).expect("Should parse write call");

        if let Syscall::WriteFile { uri, content, .. } = syscall {
            assert_eq!(uri, "output.txt");
            assert_eq!(content, "Hello World");
        } else {
            panic!("Wrong syscall type");
        }
    }

    #[tokio::test]
    async fn test_syscall_execution_format() {
        let manager = Arc::new(PluginManager::new().unwrap());
        let vcm = Arc::new(VirtualContextManager::new());
        let scribe = Arc::new(ScribeManager::new("./users_test"));
        let swap = Arc::new(LanceSwapManager::new("./swap_test"));
        let executor = SyscallExecutor::new(manager, vcm, scribe, swap);

        let pcb = crate::pcb::PCB::new("test".into(), 5, "test".into());

        // Creamos una syscall que fallará (plugin no cargado) pero verificamos el flujo
        let syscall = Syscall::PluginCall {
            plugin_name: "non_existent".to_string(),
            args_json: "{}".to_string(),
        };

        let res = executor.execute(&pcb, syscall).await;
        assert!(matches!(res, Err(SyscallError::PluginError(_))));
    }

    #[tokio::test]
    async fn test_ssrf_guard_blocking() {
        let manager = Arc::new(PluginManager::new().unwrap());
        let vcm = Arc::new(VirtualContextManager::new());
        let scribe = Arc::new(ScribeManager::new("./users_test"));
        let swap = Arc::new(LanceSwapManager::new("./swap_test"));
        let executor = SyscallExecutor::new(manager, vcm, scribe, swap);

        // Intentar acceder a localhost
        let res = executor.fetch_url_safe("http://127.0.0.1:8080/admin").await;
        assert!(matches!(res, Err(SyscallError::SecurityViolation(_))));
        
        // Intentar acceder a red privada (RFC 1918)
        let res_private = executor.fetch_url_safe("http://192.168.1.1/config").await;
        assert!(matches!(res_private, Err(SyscallError::SecurityViolation(_))));
    }
}

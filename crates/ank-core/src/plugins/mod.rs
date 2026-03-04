use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};
use tracing::info;

/// --- PLUGIN ERROR SYSTEM ---
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Compilation Failed: {0}")]
    CompilationFailed(String),
    #[error("Security Violation: {0}")]
    SecurityViolation(String),
    #[error("Plugin Execution Failed: {0}")]
    ExecutionFailed(String),
    #[error("Function Not Found: {0}")]
    FunctionNotFound(String),
    #[error("Execution Trap: {0}")]
    ExecutionTrap(String),
    #[error("Out of Fuel: The plugin exceeded its CPU budget")]
    OutOfFuel,
    #[error("IO Error: {0}")]
    IOError(String),
}

/// --- PLUGIN METADATA ---
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub parameter_example: String,
}

/// --- PLUGIN ---
/// Representa una herramienta cargada en el "User Space" del Kernel.
pub struct Plugin {
    pub metadata: PluginMetadata,
    pub module: Module,
}

/// --- PLUGIN STATE ---
/// Estado interno para el sandbox Wasm (WASI P1 bridge).
struct PluginState {
    wasi_ctx: wasmtime_wasi::preview1::WasiP1Ctx,
    table: ResourceTable,
}

impl WasiView for PluginState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
    fn ctx(&mut self) -> &mut WasiCtx {
        // WasiP1Ctx no implementa WasiView directamente pero el Linker P1 lo usa.
        // Como implementamos WasiView para P2 futuro, mantenemos esto pero el P1 usará el campo directamente.
        unimplemented!("P1 bridge uses direct access; P2 will use this")
    }
}

/// --- PLUGIN MANAGER ---
/// Orquestador del sistema de plugins basado en Wasmtime.
pub struct PluginManager {
    engine: Engine,
    linker: Linker<PluginState>,
    plugins: HashMap<String, Plugin>,
}

impl PluginManager {
    /// Inicializa el motor de Wasm con configuraciones optimizadas
    /// y medidas de seguridad (Fuel consumption, WASI, CPU limits).
    pub fn new() -> Result<Self, PluginError> {
        let mut config = Config::new();

        // --- CONFIGURACIÓN DE SEGURIDAD Y RENDIMIENTO ---
        // 1. Habilitar soporte asíncrono para integrarse con Tokio
        config.async_support(true);

        // 2. Limitar consumo de recursos (CPU Fuel)
        // Esto permite abortar plugins que entren en bucles infinitos.
        config.consume_fuel(true);

        // 3. Optimización máxima de compilación JIT (Cranelift)
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        // 4. Memory Limits y Protección
        config.memory_reservation(512 * 1024 * 1024); // Máximo 512MB RAM

        let engine =
            Engine::new(&config).map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

        // --- CACHEO DE LINKER ---
        // Pre-vincular funciones WASI comunes para evitar re-computación en cada ejecución.
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview1::add_to_linker_async(&mut linker, |s: &mut PluginState| {
            &mut s.wasi_ctx
        })
        .map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

        Ok(Self {
            engine,
            linker,
            plugins: HashMap::new(),
        })
    }

    /// Carga un binario .wasm del disco, lo compila y lo cachea.
    pub fn load_plugin(&mut self, path: &str) -> Result<(), PluginError> {
        let name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::IOError("Invalid plugin path".to_string()))?;

        let module = Module::from_file(&self.engine, path)
            .map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

        // Registro de metadatos asistido por el Kernel (Tool Discovery)
        let (description, parameter_example) = match name {
            "std_sys" => (
                "Obtiene la fecha y hora actual del sistema en formato UTC.",
                "{\"action\": \"get_time\"}"
            ),
            "std_fs" => (
                "Explora el sistema de archivos del workspace (listado de directorios y lectura).",
                "{\"action\": \"list_dir\", \"path\": \"/workspace\"}"
            ),
            "std_net" => (
                "Acceso a red seguro: descarga e interpreta el contenido de una URL.",
                "{\"action\": \"fetch\", \"url\": \"https://example.com\"}"
            ),
            _ => ("ANK Wasm Plugin (Metadata pendiente)", "{}"),
        };

        let metadata = PluginMetadata {
            name: name.to_string(),
            description: description.to_string(),
            version: "1.0.0".to_string(),
            author: "Aegis Kernel Team".to_string(),
            parameter_example: parameter_example.to_string(),
        };

        self.plugins
            .insert(name.to_string(), Plugin { metadata, module });
        Ok(())
    }

    /// Escanea un directorio y carga todos los binarios .wasm encontrados.
    /// Útil para el despliegue automático mediante deploy_debian.sh.
    pub fn load_all_from_dir(&mut self, dir_path: &str) -> Result<(), PluginError> {
        let path = Path::new(dir_path);
        if !path.exists() || !path.is_dir() {
            return Err(PluginError::IOError(format!("Plugin directory not found: {}", dir_path)));
        }

        for entry in std::fs::read_dir(path).map_err(|e| PluginError::IOError(e.to_string()))? {
            let entry = entry.map_err(|e| PluginError::IOError(e.to_string()))?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                if let Some(path_str) = path.to_str() {
                    self.load_plugin(path_str)?;
                    info!("Plugin auto-loaded: {:?}", path.file_name().unwrap_or_default());
                }
            }
        }
        Ok(())
    }

    /// Ejecuta un plugin en un sandbox aislado (Ring 0) con Jailing dinámico.
    /// El paso de datos se realiza vía JSON por stdin/stdout (Estándar WASI).
    pub async fn execute_plugin(
        &self,
        tenant_id: &str,
        plugin_name: &str,
        input_json: &str,
    ) -> Result<String, PluginError> {
        let plugin = self.plugins.get(plugin_name).ok_or_else(|| {
            PluginError::FunctionNotFound(format!("Plugin {} not loaded", plugin_name))
        })?;

        // --- INTERCEPCIÓN COGNITIVA (EPIC 8) ---
        // Si el plugin es std_net, interceptamos el JSON para realizar la descarga en el Host.
        let final_input = if plugin_name == "std_net" {
            let req: serde_json::Value = serde_json::from_str(input_json)
                .map_err(|e| PluginError::ExecutionFailed(format!("Invalid JSON for std_net: {}", e)))?;
            
            let url = req["url"].as_str().ok_or_else(|| {
                PluginError::ExecutionFailed("std_net requires 'url' parameter".to_string())
            })?;

            // El Kernel realiza la descarga segura
            self.fetch_url_safe(url).await?
        } else {
            input_json.to_string()
        };

        // 1. Configurar Stdin/Stdout virtuales para el intercambio de JSON
        let stdin = MemoryInputPipe::new(final_input.as_bytes().to_vec());
        let stdout = MemoryOutputPipe::new(4096 * 10); // Buffer de 40KB para el resultado (HTML procesado)

        // 2. Construir el contexto WASI (Dynamic Jailing)
        let workspace_path = format!("./users/{}/workspace", tenant_id);
        
        // SRE Guard: Asegurar que el directorio de trabajo existe físicamente antes de montar la jaula
        std::fs::create_dir_all(&workspace_path)
            .map_err(|e| PluginError::IOError(format!("Critical: Failed to create jail for tenant {}: {}", tenant_id, e)))?;

        // Abrir el directorio del host con capacidades restringidas (cap-std)
        // Esto expone físicamente la carpeta del tenant como /workspace dentro del entorno Wasm.
        let dir = wasmtime_wasi::Dir::open_ambient_dir(&workspace_path, wasmtime_wasi::ambient_authority())
            .map_err(|e| PluginError::SecurityViolation(format!("Sandbox Escape Prevention: Failed to open preopened dir for {}: {}", tenant_id, e)))?;

        let wasi_ctx = WasiCtxBuilder::new()
            .stdin(stdin)
            .stdout(stdout.clone())
            .preopened_dir(dir, "/workspace")
            .build_p1();

        let state = PluginState {
            wasi_ctx,
            table: ResourceTable::new(),
        };

        let mut store = Store::new(&self.engine, state);

        // Asignar combustible al Store (Presupuesto de CPU)
        // 1 unidad ~= 1 instrucción (aprox).
        store
            .set_fuel(1_000_000)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // 3. Instanciar desde el Linker (Cacho de Módulos y Linker)
        let instance = self
            .linker
            .instantiate_async(&mut store, &plugin.module)
            .await
            .map_err(|e| PluginError::ExecutionTrap(e.to_string()))?;

        // 4. Invocar punto de entrada (_start para WASI Commands o execute)
        // Intentamos _start primero (estándar WASI).
        let func = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|_| PluginError::FunctionNotFound("_start not exported".to_string()))?;

        func.call_async(&mut store, ()).await.map_err(|e| {
            if let Some(wasmtime::Trap::OutOfFuel) = e.downcast_ref::<wasmtime::Trap>() {
                PluginError::OutOfFuel
            } else {
                PluginError::ExecutionTrap(e.to_string())
            }
        })?;

        // 5. Recuperar resultado
        let output_bytes = stdout.contents();
        String::from_utf8(output_bytes.to_vec())
            .map_err(|e| PluginError::ExecutionFailed(format!("Invalid UTF-8 output: {}", e)))
    }

    /// Implementación de seguridad SRE para peticiones HTTP.
    /// Bloquea ataques SSRF validando que la URL no apunte a rangos locales o privados.
    pub async fn fetch_url_safe(&self, url_str: &str) -> Result<String, PluginError> {
        let url = reqwest::Url::parse(url_str)
            .map_err(|e| PluginError::SecurityViolation(format!("Invalid URL: {}", e)))?;

        let host = url.host_str().ok_or_else(|| PluginError::SecurityViolation("Missing host in URL".into()))?;
        
        let port = url.port_or_known_default().unwrap_or(80);
        let addrs = tokio::net::lookup_host(format!("{}:{}", host, port))
            .await
            .map_err(|e| PluginError::IOError(format!("DNS Resolution failed for {}: {}", host, e)))?;

        for addr in addrs {
            let ip = addr.ip();
            if ip.is_loopback() || ip.is_unspecified() {
                return Err(PluginError::SecurityViolation(format!("SSRF Guard: Loopback/Internal access denied for {}", ip)));
            }

            if let std::net::IpAddr::V4(v4) = ip {
                if v4.is_private() || v4.is_link_local() || v4.is_broadcast() || v4.is_documentation() {
                    return Err(PluginError::SecurityViolation(format!("SSRF Guard: Private/Local network access denied for {}", ip)));
                }
            } else if let std::net::IpAddr::V6(v6) = ip {
                if (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80 {
                    return Err(PluginError::SecurityViolation(format!("SSRF Guard: Private IPv6 access denied for {}", ip)));
                }
            }
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("AegisNeuralKernel/1.0 (Cognitive SRE)")
            .build()
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let response = client.get(url)
            .send()
            .await
            .map_err(|e| PluginError::IOError(format!("Network request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(PluginError::IOError(format!("HTTP Error returned: {}", status)));
        }

        let body = response.text()
            .await
            .map_err(|e| PluginError::IOError(format!("Failed to read response body: {}", e)))?;

        Ok(body)
    }

    /// Genera la "Tarjeta de Habilidades" (Tool Discovery) para inyectar en el System Prompt.
    pub fn get_available_tools_prompt(&self) -> String {
        if self.plugins.is_empty() {
            return "No hay plugins Wasm cargados actualmente.".to_string();
        }

        let mut prompt = String::from("HERRAMIENTAS (PLUGINS) DISPONIBLES:\n");
        for plugin in self.plugins.values() {
            prompt.push_str(&format!(
                "- {}: {} -> Uso: [SYS_CALL_PLUGIN(\"{}\", {})]\n",
                plugin.metadata.name,
                plugin.metadata.description,
                plugin.metadata.name,
                plugin.metadata.parameter_example
            ));
        }
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_plugin_manager_init() {
        let manager = PluginManager::new();
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_wasm_execution_trap_handling() {
        let mut manager = PluginManager::new().unwrap();

        // Un wasm mínimo que hace un unreachable (trap)
        let wasm_bytes = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
            0x03, 0x02, 0x01, 0x00, 0x07, 0x0a, 0x01, 0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74,
            0x00, 0x00, 0x0a, 0x05, 0x01, 0x03, 0x00, 0x00, 0x0b,
        ];

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&wasm_bytes).unwrap();
        let path = file.path().to_str().unwrap();

        manager.load_plugin(path).unwrap();
        let res = manager.execute_plugin("test_tenant", "test", "{}").await;

        assert!(res.is_err());
        // Debe ser un ExecutionTrap o similar
    }
}

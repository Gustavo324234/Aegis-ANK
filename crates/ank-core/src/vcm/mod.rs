use crate::vcm::swap::LanceSwapManager;
use std::path::{Component, Path};
use thiserror::Error;
use tracing::warn;

pub mod swap;

/// --- VCM ERROR SYSTEM ---
#[derive(Error, Debug, Clone)]
pub enum VCMError {
    #[error("Path Traversal Detected: attempt to access {0} outside sandbox")]
    PathTraversalDetected(String),
    #[error("Context Overflow: assembled context exceeds limit of {0} tokens")]
    ContextOverflow(usize),
    #[error("File Not Found: {0}")]
    FileNotFound(String),
    #[error("IO Error: {0}")]
    IOError(String),
    #[error("File too large: {0} exceeds {1} bytes")]
    FileTooLarge(String, u64),
}

const SYSTEM_INSTRUCTIONS: &str = "### SYSTEM: Aegis Neural Kernel VCM ###\nYou are an auxiliary cognitive module of the Aegis Neural Kernel. \
Use the provided context to fulfill the instruction accurately.";

/// Límite de seguridad para evitar cargar archivos masivos en la ventana de atención.
/// Archivos mayores a 2MB se consideran fuera de la capacidad de 'working memory' estándar.
const MAX_FILE_SIZE_BYTES: u64 = 2 * 1024 * 1024;

/// --- VIRTUAL CONTEXT MANAGER ---
/// El VCM es responsable de construir la \"ventana de atención\" (Context Window)
/// para el LLM, agregando instrucciones L1, referencias L2 y memoria swap L3.
pub struct VirtualContextManager;

impl VirtualContextManager {
    pub fn new() -> Self {
        Self
    }

    /// Ensambla el contexto final a partir de un PCB y acceso a la memoria L3.
    /// Resuelve las referencias de memoria y aplica límites de tokens.
    /// Estructura: [SYSTEM_INSTRUCTIONS] + \n + [L2_CONTEXT] + \n + [L3_MEMORY] + \n + [L1_INSTRUCTION]
    pub async fn assemble_context(
        &self,
        pcb: &PCB,
        swap_manager: &LanceSwapManager,
        token_limit: usize,
    ) -> Result<String, VCMError> {
        // 1. Calcular base (System + L1) de forma eficiente
        let l1_prompt = &pcb.memory_pointers.l1_instruction;
        let base_tokens = estimate_tokens(SYSTEM_INSTRUCTIONS)
            + estimate_tokens("## INSTRUCTION\n")
            + estimate_tokens(l1_prompt)
            + 4; // Margen para saltos de línea y separadores

        if base_tokens > token_limit {
            return Err(VCMError::ContextOverflow(token_limit));
        }

        // Pre-asignamos buffer con una estimación agresiva para evitar re-allocations
        // 1 token ~= 4 bytes.
        let mut final_context = String::with_capacity(token_limit * 4);

        // Empezamos el ensamblaje directamente en el buffer final
        final_context.push_str(SYSTEM_INSTRUCTIONS);
        final_context.push_str("\n\n");

        let mut current_tokens = base_tokens;
        let mut has_l2 = false;
        let mut l3_added = false;

        // 2. Procesar L2 Context References
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
        let tenant_root = format!("./users/{}/workspace", tenant_id);

        for ref_uri in &pcb.memory_pointers.l2_context_refs {
            if ref_uri.starts_with("file://") {
                let path_part = &ref_uri[7..];
                if !is_safe_path(tenant_id, path_part) {
                    return Err(VCMError::PathTraversalDetected(path_part.to_string()));
                }

                // Rutear al Workspace del Tenant
                let full_path = Path::new(&tenant_root).join(path_part);

                // OPTIMIZACIÓN: Verificar metadatos antes de leer
                let metadata = match tokio::fs::metadata(&full_path).await {
                    Ok(m) => m,
                    Err(e) => return Err(VCMError::IOError(format!("{}: {}", path_part, e))),
                };

                if metadata.len() > MAX_FILE_SIZE_BYTES {
                    warn!(path = %path_part, size = %metadata.len(), "File too large for VCM, skipping.");
                    if !has_l2 {
                        final_context.push_str("## ATTACHED CONTEXT\n");
                        has_l2 = true;
                    }
                    final_context.push_str(&format!(
                        "[SYSTEM: {} omitido por tamaño excesivo]\n",
                        ref_uri
                    ));
                    continue;
                }

                // Estimación rápida basada en tamaño de archivo antes de leer
                let estimated_entry_tokens = (metadata.len() as usize / 4) + 50; // +50 por el header

                if current_tokens + estimated_entry_tokens > token_limit {
                    if !has_l2 {
                        final_context.push_str("## ATTACHED CONTEXT\n");
                        has_l2 = true;
                    }
                    final_context.push_str(&format!(
                        "[SYSTEM: {} omitido por falta de memoria]\n",
                        ref_uri
                    ));
                    continue;
                }

                // Lectura asíncrona (Solo si sabemos que cabe y es seguro)
                let content = match tokio::fs::read_to_string(&full_path).await {
                    Ok(c) => c,
                    Err(e) => return Err(VCMError::IOError(format!("{}: {}", path_part, e))),
                };

                if !has_l2 {
                    final_context.push_str("## ATTACHED CONTEXT\n");
                    has_l2 = true;
                }

                final_context.push_str("[File: ");
                final_context.push_str(path_part);
                final_context.push_str("]\n");
                final_context.push_str(&content);
                final_context.push_str("\n");

                current_tokens += estimate_tokens(&content) + 10;
            }
        }

        // 3. Procesar L3 Semantic Memory (Swap)
        // REGLA SRE: Solo si queda espacio después de L2.
        if current_tokens < token_limit && !pcb.memory_pointers.swap_refs.is_empty() {
            for swap_query in &pcb.memory_pointers.swap_refs {
                // Parseamos el query. Si empieza con 'vec:', lo tratamos como vector.
                // Si no, usamos un vector dummy (esto se integrará con un embedding driver luego).
                let vector = if swap_query.starts_with("vec:") {
                    swap_query[4..]
                        .split(',')
                        .filter_map(|s| s.trim().parse::<f32>().ok())
                        .collect::<Vec<f32>>()
                } else {
                    // TODO: Reemplazar por llamada a Embedding Server
                    vec![0.0; 128]
                };

                if vector.is_empty() {
                    continue;
                }

                if let Ok(fragments) = swap_manager.search(tenant_id, vector, 3).await {
                    for fragment in fragments {
                        let fragment_tokens = estimate_tokens(&fragment.text) + 20;

                        if current_tokens + fragment_tokens > token_limit {
                            break;
                        }

                        if !l3_added {
                            final_context.push_str("\n## L3 SEMANTIC MEMORY\n");
                            l3_added = true;
                        }

                        final_context.push_str(&format!("[Memory ID: {}]\n", fragment.id));
                        final_context.push_str(&fragment.text);
                        final_context.push_str("\n");

                        current_tokens += fragment_tokens;
                    }
                }

                if current_tokens >= token_limit {
                    break;
                }
            }
        }

        // 4. Instrucción Final (L1)
        if has_l2 || l3_added {
            final_context.push_str("\n");
        }
        final_context.push_str("## INSTRUCTION\n");
        final_context.push_str(l1_prompt);
        final_context.push_str("\n");

        Ok(final_context)
    }
}

/// Heurística simple: 4 caracteres equivalen aproximadamente a 1 token.
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Auditoría de Seguridad: Previene el acceso a archivos fuera del sandbox de trabajo.
/// Verifica que no existan retrocesos de directorio ("..") que escapen del root permitido.
fn is_safe_path(_tenant_id: &str, path_str: &str) -> bool {
    let path = Path::new(path_str);

    // 1. Prohibir rutas absolutas por seguridad (aislamiento)
    if path.is_absolute() {
        return false;
    }

    // 2. Normalizar componentes y verificar profundidad
    let mut depth: i32 = 0;
    for component in path.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false; // Intento de salir del directorio base (Root Escape)
                }
            }
            Component::CurDir => continue,
            _ => return false, // No permitimos RootDir (ya cubierto), Prefix o similar.
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::PCB;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_assemble_basic_context() {
        let vcm = VirtualContextManager::new();
        let swap = LanceSwapManager::new("./test_users"); // Mock
        let pcb = PCB::new("TestProcess".into(), 5, "Summarize this".into());

        // Límite generoso
        let context = vcm.assemble_context(&pcb, &swap, 1000).await.unwrap();

        assert!(context.contains("SYSTEM: Aegis Neural Kernel VCM"));
        assert!(context.contains("Summarize this"));
        // El orden debe ser SYSTEM -> L1 (sin L2 en esta prueba básica)
    }

    #[tokio::test]
    async fn test_vcm_file_omission_on_overflow() {
        let vcm = VirtualContextManager::new();
        let swap = LanceSwapManager::new("./test_users");

        // Crear estructura de directorios para el tenant default
        let workspace_path = "./users/default/workspace";
        tokio::fs::create_dir_all(workspace_path).await.unwrap();

        // Crear un archivo temporal con ruta relativa dentro del workspace del tenant
        let file_name = "test_overflow_dummy.txt";
        let full_path = std::path::Path::new(workspace_path).join(file_name);
        
        let mut file = std::fs::File::create(&full_path).unwrap();
        let large_content = "X".repeat(2000); // ~500 tokens
        file.write_all(large_content.as_bytes()).unwrap();

        let mut pcb = PCB::new("HeavyProc".into(), 5, "Small task".into());
        pcb.memory_pointers
            .l2_context_refs
            .push(format!("file://{}", file_name));

        // Límite pequeño que no permite el archivo pero sí el resto
        let context = vcm.assemble_context(&pcb, &swap, 100).await.unwrap();

        // Limpiar
        let _ = std::fs::remove_file(&full_path);

        assert!(
            context.contains("omitido por falta de memoria")
                || context.contains("omitido por tamaño excesivo")
        );
        assert!(!context.contains(&large_content));
        assert!(context.contains("Small task"));
    }

    #[tokio::test]
    async fn test_vcm_l3_memory_injection() {
        let vcm = VirtualContextManager::new();
        let swap = LanceSwapManager::new("./test_users");
        // In a real test, we would add fragments to LanceDB.
        // For now, search returns an empty list since the DB is empty.

        let mut pcb = PCB::new("SwapProc".into(), 5, "Check memory".into());
        pcb.memory_pointers.swap_refs.push("vec:0.1,0.2".into());

        let context = vcm.assemble_context(&pcb, &swap, 1000).await.unwrap();

        // No debería fallar, aunque la lista esté vacía.
        assert!(context.contains("Check memory"));
    }

    #[test]
    fn test_path_traversal_audit() {
        assert!(is_safe_path("tenant_1", "docs/contract.md"));
        assert!(!is_safe_path("tenant_1", "../etc/passwd"));
        assert!(!is_safe_path("tenant_1", "/absolute/path"));
    }
}

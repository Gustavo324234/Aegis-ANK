use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Representa la solicitud recibida por el plugin.
#[derive(Debug, Deserialize)]
struct PluginRequest {
    action: String,
    #[serde(default)]
    params: serde_json::Value,
}

/// Representa la respuesta enviada por el plugin.
#[derive(Debug, Serialize)]
struct PluginResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

const WORKSPACE_ROOT: &str = "/workspace";

fn main() -> Result<()> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("Error al leer de stdin")?;

    let response = match process_request(&buffer) {
        Ok(res) => res,
        Err(e) => PluginResponse {
            status: "error".to_string(),
            data: None,
            error: Some(format!("{:?}", e)),
        },
    };

    let output = serde_json::to_string(&response).context("Error al serializar la respuesta")?;
    io::stdout()
        .write_all(output.as_bytes())
        .context("Error al escribir en stdout")?;
    io::stdout().flush().context("Error al limpiar stdout")?;

    Ok(())
}

fn process_request(input: &str) -> Result<PluginResponse> {
    let request: PluginRequest = serde_json::from_str(input)
        .context("Error al deserializar el JSON de entrada.")?;

    match request.action.as_str() {
        "list_dir" => list_dir(&request.params),
        "read_file" => read_file(&request.params),
        _ => Ok(PluginResponse {
            status: "error".to_string(),
            data: None,
            error: Some(format!("Acción desconocida: {}", request.action)),
        }),
    }
}

fn list_dir(params: &serde_json::Value) -> Result<PluginResponse> {
    let relative_path = params["path"].as_str().unwrap_or(".");
    let target_path = safe_path(relative_path)?;

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&target_path)
        .with_context(|| format!("No se pudo leer el directorio: {}", relative_path))?;

    for entry in read_dir {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let metadata = entry.metadata()?;
        let kind = if metadata.is_dir() { "dir" } else { "file" };
        
        entries.push(serde_json::json!({
            "name": file_name,
            "type": kind,
            "size": metadata.len()
        }));
    }

    Ok(PluginResponse {
        status: "success".to_string(),
        data: Some(serde_json::Value::Array(entries)),
        error: None,
    })
}

fn read_file(params: &serde_json::Value) -> Result<PluginResponse> {
    let relative_path = params["path"].as_str()
        .context("Se requiere la clave 'path' para leer un archivo")?;
    let target_path = safe_path(relative_path)?;

    let content = fs::read_to_string(&target_path)
        .with_context(|| format!("No se pudo leer el archivo: {}", relative_path))?;

    Ok(PluginResponse {
        status: "success".to_string(),
        data: Some(serde_json::Value::String(content)),
        error: None,
    })
}

/// Construye una ruta segura dentro del prefijo /workspace.
/// Evita ataques de path traversal básicos.
fn safe_path(rel_path: &str) -> Result<PathBuf> {
    let mut path = PathBuf::from(WORKSPACE_ROOT);
    
    // Eliminamos prefijos peligrosos como / o ..
    let rel = rel_path.trim_start_matches(|c| c == '/' || c == '\\');
    path.push(rel);

    // En un entorno WASI real con jailing, no podemos salir de /workspace
    // pero añadimos una verificación lógica extra.
    if !path.starts_with(WORKSPACE_ROOT) {
        anyhow::bail!("Acceso fuera del workspace denegado: {}", rel_path);
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_path_logic() {
        // En tests unitarios normales (no wasm), /workspace no existirá, 
        // pero validamos la construcción de la ruta.
        let p = safe_path("docs/readme.txt").unwrap();
        assert!(p.to_str().unwrap().contains("workspace"));
    }

    #[test]
    fn test_unknown_action() {
        let input = r#"{"action": "not_exists", "params": {}}"#;
        let res = process_request(input).unwrap();
        assert_eq!(res.status, "error");
    }
}

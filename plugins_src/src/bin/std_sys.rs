use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

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

fn main() -> Result<()> {
    // Leemos de stdin hasta el final. En un entorno WASI real, esto suele ser un buffer JSON completo.
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("Error al leer de stdin")?;

    // Procesamos la solicitud
    let response = match process_request(&buffer) {
        Ok(res) => res,
        Err(e) => PluginResponse {
            status: "error".to_string(),
            data: None,
            error: Some(format!("{:?}", e)),
        },
    };

    // Escribimos el resultado en stdout
    let output = serde_json::to_string(&response).context("Error al serializar la respuesta")?;
    io::stdout()
        .write_all(output.as_bytes())
        .context("Error al escribir en stdout")?;
    io::stdout().flush().context("Error al limpiar stdout")?;

    Ok(())
}

/// Lógica central de procesamiento para evitar mezclar IO con lógica de negocio.
fn process_request(input: &str) -> Result<PluginResponse> {
    let request: PluginRequest = serde_json::from_str(input)
        .context("Error al deserializar el JSON de entrada. Se esperaba un objeto con la clave 'action'.")?;

    match request.action.as_str() {
        "get_time" => {
            let now = Utc::now();
            Ok(PluginResponse {
                status: "success".to_string(),
                data: Some(serde_json::to_value(now).context("Error al convertir Utc::now() a JSON")?),
                error: None,
            })
        }
        _ => Ok(PluginResponse {
            status: "error".to_string(),
            data: None,
            error: Some(format!("Acción desconocida: {}", request.action)),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_get_time() {
        let input = r#"{"action": "get_time"}"#;
        let result = process_request(input).expect("Debería procesar de forma segura");
        assert_eq!(result.status, "success");
        assert!(result.data.is_some());
        
        // Verificamos que sea una fecha válida (formato ISO 8601 que usa chrono por defecto en Serde)
        let date_str = result.data.unwrap().as_str().unwrap().to_string();
        assert!(date_str.contains("Z")); // UTC
    }

    #[test]
    fn test_unknown_action() {
        let input = r#"{"action": "unknown_cmd"}"#;
        let result = process_request(input).expect("Debería retornar un PluginResponse de error");
        assert_eq!(result.status, "error");
        assert!(result.error.unwrap().contains("Acción desconocida"));
    }

    #[test]
    fn test_invalid_json() {
        let input = r#"{"invalid": "json"}"#;
        let result = process_request(input);
        assert!(result.is_err(), "Debería fallar la validación si falta la clave 'action'");
    }
}

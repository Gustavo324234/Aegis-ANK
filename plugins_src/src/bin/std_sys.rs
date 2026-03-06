use anyhow::{Context, Result};
use chrono::Utc;
use aegis_sdk::{PluginRequest, PluginResponse, PluginMetadata, run_plugin};

fn main() -> Result<()> {
    let metadata = PluginMetadata {
        name: "std_sys".to_string(),
        description: "Standard System Plugin for Aegis OS (Time & OS Info)".to_string(),
        example_json: serde_json::json!({
            "action": "get_time"
        }),
    };

    run_plugin(metadata, process_request)
}

fn process_request(request: &PluginRequest) -> Result<PluginResponse> {
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
        let req = PluginRequest {
            action: "get_time".to_string(),
            params: serde_json::Value::Null,
        };
        let result = process_request(&req).expect("Debería procesar de forma segura");
        assert_eq!(result.status, "success");
        assert!(result.data.is_some());
        
        // Verificamos que sea una fecha válida (formato ISO 8601 que usa chrono por defecto en Serde)
        let date_str = result.data.unwrap().as_str().unwrap().to_string();
        assert!(date_str.contains("Z")); // UTC
    }

    #[test]
    fn test_unknown_action() {
        let req = PluginRequest {
            action: "unknown_cmd".to_string(),
            params: serde_json::Value::Null,
        };
        let result = process_request(&req).expect("Debería retornar un PluginResponse de error");
        assert_eq!(result.status, "error");
        assert!(result.error.unwrap().contains("Acción desconocida"));
    }
}

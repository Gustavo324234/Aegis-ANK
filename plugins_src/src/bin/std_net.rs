use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// Representa la solicitud recibida por el plugin.
/// En el caso de std_net interceptado, el Kernel pasará el HTML directo como bloque de texto
/// o envolverá el resultado en un JSON. Para máxima flexibilidad, intentaremos parsear
/// como JSON pero si falla asumiremos que es HTML crudo inyectado por el Kernel.
#[derive(Debug, Deserialize)]
struct PluginRequest {
    action: String,
    #[serde(default)]
    html: String,
}

#[derive(Debug, Serialize)]
struct PluginResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn main() -> Result<()> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("Error al leer de stdin")?;

    // Lógica de limpieza: 
    // Si el Kernel inyectó el HTML directamente (después de interceptar la URL),
    // el buffer contendrá el HTML.
    let cleaned_text = clean_html(&buffer);

    let response = PluginResponse {
        status: "success".to_string(),
        data: Some(serde_json::Value::String(cleaned_text)),
        error: None,
    };

    let output = serde_json::to_string(&response).context("Error al serializar la respuesta")?;
    io::stdout()
        .write_all(output.as_bytes())
        .context("Error al escribir en stdout")?;
    io::stdout().flush().context("Error al limpiar stdout")?;

    Ok(())
}

/// Limpiador de HTML ultra-ligero para Wasm (Cero dependencias externas).
/// Usa una máquina de estados básica para omitir todo lo que esté entre < y >.
fn clean_html(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script_or_style = false;
    
    // Simplificación: no manejamos tags anidados complejos ni comentarios de forma perfecta,
    // pero para extraer texto legible por una IA es suficiente.
    
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                in_tag = true;
                // Detectar inicio de <script o <style para ignorar su contenido
                let mut tag_acc = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c == '>' || next_c.is_whitespace() { break; }
                    tag_acc.push(chars.next().unwrap().to_ascii_lowercase());
                }
                if tag_acc == "script" || tag_acc == "style" {
                    in_script_or_style = true;
                }
            }
            '>' => {
                in_tag = false;
                // Si cerramos un tag de script o style, debemos buscar el cierre </script>...
                // Pero para esta versión v1 simplificada, simplemente reiniciamos flags.
            }
            _ => {
                if !in_tag && !in_script_or_style {
                    output.push(c);
                }
            }
        }
    }
    
    // Post-procesamiento: Colapsar espacios y eliminar líneas vacías excesivas
    output.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_html() {
        let html = "<html><body><h1>Hola</h1><p>Mundo</p><style>body { color: red; }</style></body></html>";
        let cleaned = clean_html(html);
        assert!(cleaned.contains("Hola"));
        assert!(cleaned.contains("Mundo"));
        assert!(!cleaned.contains("color: red"));
        assert!(!cleaned.contains("<html>"));
    }
}

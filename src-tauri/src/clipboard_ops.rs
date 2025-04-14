use std::error::Error;

// Función para copiar texto al portapapeles
// En Tauri, esto se maneja a través de los comandos de Tauri en lugar de usar la biblioteca clipboard directamente
pub fn copy_to_clipboard(_text: &str) -> Result<(), Box<dyn Error>> {
    // En la implementación real, esta función no se usará directamente
    // ya que Tauri proporciona su propia API para acceder al portapapeles
    // Sin embargo, la mantenemos para compatibilidad con el código existente
    Ok(())
}

// Función para obtener texto del portapapeles
// En Tauri, esto se maneja a través de los comandos de Tauri
pub fn get_from_clipboard() -> Result<String, Box<dyn Error>> {
    // En la implementación real, esta función no se usará directamente
    // ya que Tauri proporciona su propia API para acceder al portapapeles
    Ok(String::new())
}

// Función para formatear el contenido de los archivos antes de copiarlo al portapapeles
pub fn format_file_contents(paths: &[String], contents: &[String], include_filenames: bool) -> String {
    let mut concatenated = String::new();
    
    for (i, (path, content)) in paths.iter().zip(contents.iter()).enumerate() {
        if i > 0 {
            concatenated.push_str("\n\n");
        }
        
        if include_filenames {
            concatenated.push_str(&format!("// Archivo: {}\n", path));
        }
        
        concatenated.push_str(content);
    }
    
    concatenated
}

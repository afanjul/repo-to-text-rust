use std::fs;
use std::io::{self, Read};

// Función para leer el contenido de un archivo
pub fn read_file(path: &str) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

// Encuentra el directorio base común para un conjunto de rutas de archivo
fn find_common_base_dir(paths: &[String]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    
    if paths.len() == 1 {
        // Si solo hay un archivo, devolver su directorio
        let path = &paths[0];
        if let Some(last_slash) = path.rfind('/') {
            return path[0..last_slash].to_string();
        }
        return String::new();
    }

    // Dividir todas las rutas en componentes
    let path_components: Vec<Vec<&str>> = paths
        .iter()
        .map(|path| path.split('/').collect())
        .collect();

    // Encontrar el prefijo común
    let mut common_prefix = Vec::new();
    let first_path = &path_components[0];

    'outer: for (i, component) in first_path.iter().enumerate() {
        for path in &path_components[1..] {
            if i >= path.len() || path[i] != *component {
                break 'outer;
            }
        }
        common_prefix.push(*component);
    }

    // Reconstruir la ruta base común
    if common_prefix.is_empty() {
        String::new()
    } else {
        common_prefix.join("/")
    }
}

// Función para leer múltiples archivos y combinar su contenido
pub fn read_files(paths: &[String], root_dir: &str) -> io::Result<String> {
    let mut combined_content = String::new();
    
    // Asegurarse de que el directorio raíz termine con '/'
    let root_dir = if root_dir.ends_with('/') {
        root_dir.to_string()
    } else {
        format!("{}/", root_dir)
    };
    
    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            combined_content.push_str("\n\n");
        }
        
        // Calcular la ruta relativa al directorio raíz
        let relative_path = if path.starts_with(&root_dir) {
            path[root_dir.len()..].to_string()
        } else {
            // Si no podemos calcular la ruta relativa, usamos la ruta completa
            path.clone()
        };
        
        // Añadir el encabezado con la ruta relativa
        combined_content.push_str(&format!("// ---- File: {} ----\n", relative_path));
        
        // Leer y añadir el contenido del archivo
        match read_file(path) {
            Ok(content) => {
                combined_content.push_str(&content);
            },
            Err(e) => {
                combined_content.push_str(&format!("Error al leer el archivo: {}", e));
            }
        }
    }
    
    Ok(combined_content)
}

// Función para verificar si un archivo es legible como texto
pub fn is_text_file(path: &str) -> bool {
    if let Ok(mut file) = fs::File::open(path) {
        let mut buffer = [0; 1024];
        if let Ok(bytes_read) = file.read(&mut buffer) {
            if bytes_read == 0 {
                return true; // Archivo vacío, considerado como texto
            }
            
            // Verificar si el contenido parece ser texto
            let null_bytes = buffer[..bytes_read].iter().filter(|&&b| b == 0).count();
            return null_bytes < bytes_read / 10; // Menos del 10% de bytes nulos
        }
    }
    
    false
}

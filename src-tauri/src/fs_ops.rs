use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

pub fn scan_directory(path: &str) -> Result<Vec<FileEntry>, io::Error> {
    let mut entries = Vec::new();
    
    for entry in WalkDir::new(path)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path_obj = entry.path();
        let metadata = match path_obj.metadata() {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        
        let file_name = path_obj.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
            
        entries.push(FileEntry {
            path: path_obj.to_string_lossy().into_owned(),
            name: file_name,
            is_dir: metadata.is_dir(),
            size: if metadata.is_file() { metadata.len() } else { 0 },
        });
    }
    
    // Ordenar: primero directorios, luego archivos, ambos alfabéticamente
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    
    Ok(entries)
}

pub fn get_last_directory() -> Option<String> {
    match fs::read_to_string("last_directory.txt") {
        Ok(content) => Some(content),
        Err(_) => None,
    }
}

pub fn save_last_directory(path: &str) -> Result<(), io::Error> {
    fs::write("last_directory.txt", path)
}

/// Genera un árbol de archivos en formato texto para los archivos seleccionados
/// 
/// El formato es similar a:
/// ```
/// <file_map>
/// /ruta/base
/// ├── archivo1.txt
/// ├── archivo2.txt
/// └── archivo3.txt
/// </file_map>
/// ```
pub fn generate_file_tree(files: &[String], root_dir: &str) -> Result<String, String> {
    if files.is_empty() {
        return Ok(String::from("<file_map>\nNo hay archivos seleccionados\n</file_map>"));
    }

    // Asegurarse de que el directorio raíz termine con '/'
    let root_dir = if root_dir.ends_with('/') {
        root_dir.to_string()
    } else {
        format!("{}/", root_dir)
    };

    // Ordenar los archivos para que el árbol se vea bien
    let mut sorted_files = files.to_vec();
    sorted_files.sort();

    // Generar el árbol
    let mut tree = String::from("<file_map>\n");
    // Mostrar el directorio raíz como base
    tree.push_str(root_dir.trim_end_matches('/'));
    tree.push('\n');

    // Obtener solo los nombres de archivo relativos a la base
    let file_names: Vec<String> = sorted_files.iter()
        .map(|path| {
            if path.starts_with(&root_dir) {
                path[root_dir.len()..].to_string()
            } else {
                path.clone()
            }
        })
        .collect();

    // Añadir cada archivo al árbol con el prefijo adecuado
    for (i, name) in file_names.iter().enumerate() {
        let prefix = if i == file_names.len() - 1 {
            "└── " // Último elemento
        } else {
            "├── " // Elementos intermedios
        };
        
        tree.push_str(&format!("{}{}\n", prefix, name));
    }

    tree.push_str("</file_map>");
    Ok(tree)
}

/// Encuentra el directorio base común para un conjunto de rutas de archivo
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

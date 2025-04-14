use clipboard::{ClipboardProvider, ClipboardContext};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

pub fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn Error>> {
    let mut ctx: ClipboardContext = ClipboardProvider::new()?;
    ctx.set_contents(text.to_owned())?;
    Ok(())
}

pub fn get_from_clipboard() -> Result<String, Box<dyn Error>> {
    let mut ctx: ClipboardContext = ClipboardProvider::new()?;
    let contents = ctx.get_contents()?;
    Ok(contents)
}

pub fn copy_file_contents(contents: &[(String, String)], include_filenames: bool) -> Result<(), Box<dyn Error>> {
    let mut concatenated = String::new();
    for (path, content) in contents {
        if include_filenames {
            concatenated.push_str(&format!("// File: {}\n", path));
        }
        concatenated.push_str(content);
        concatenated.push_str("\n\n");
    }
    copy_to_clipboard(&concatenated)
}

/// Generates a text-based directory tree representation
fn generate_file_tree(file_paths: &[String]) -> String {
    if file_paths.is_empty() {
        return String::new();
    }

    // Find the common root directory
    let paths: Vec<PathBuf> = file_paths.iter().map(|p| PathBuf::from(p)).collect();
    let mut common_root = paths[0].clone();
    
    // Find the common parent directory of all files
    for path in &paths[1..] {
        while !path.starts_with(&common_root) {
            if let Some(parent) = common_root.parent() {
                common_root = parent.to_path_buf();
            } else {
                break;
            }
        }
    }
    
    let common_root_str = common_root.to_string_lossy().to_string();
    
    // Build a directory structure
    let mut dir_structure: HashMap<String, Vec<String>> = HashMap::new();
    
    for path in file_paths {
        let path_obj = Path::new(path);
        if let Some(parent) = path_obj.parent() {
            let parent_str = parent.to_string_lossy().to_string();
            let file_name = path_obj.file_name().unwrap_or_default().to_string_lossy().to_string();
            
            dir_structure.entry(parent_str)
                .or_insert_with(Vec::new)
                .push(file_name);
        }
    }
    
    // Sort directories and files
    for files in dir_structure.values_mut() {
        files.sort();
    }
    
    // Generate the tree
    let mut result = String::new();
    result.push_str("<file_map>\n");
    result.push_str(&common_root_str);
    result.push_str("\n");
    
    // Helper function to build the tree recursively
    fn build_tree(
        dir: &str,
        dir_structure: &HashMap<String, Vec<String>>,
        result: &mut String,
        prefix: &str,
        is_last_dir: bool,
        processed_dirs: &mut Vec<String>,
    ) {
        if processed_dirs.contains(&dir.to_string()) {
            return;
        }
        
        processed_dirs.push(dir.to_string());
        
        // Get files in this directory
        let empty_vec = Vec::new();
        let files = dir_structure.get(dir).unwrap_or(&empty_vec);
        
        // Get subdirectories
        let mut subdirs: Vec<String> = dir_structure.keys()
            .filter(|&k| k != dir && k.starts_with(dir) && k.chars().filter(|&c| c == '/').count() == dir.chars().filter(|&c| c == '/').count() + 1)
            .cloned()
            .collect();
        subdirs.sort();
        
        // Process files
        for (i, file) in files.iter().enumerate() {
            let is_last = i == files.len() - 1 && subdirs.is_empty();
            let line_prefix = if is_last { "└── " } else { "├── " };
            result.push_str(&format!("{}{}{}\n", prefix, line_prefix, file));
        }
        
        // Process subdirectories
        for (i, subdir) in subdirs.iter().enumerate() {
            let is_last = i == subdirs.len() - 1;
            let line_prefix = if is_last { "└── " } else { "├── " };
            
            let dir_name = Path::new(subdir)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
                
            result.push_str(&format!("{}{}{}\n", prefix, line_prefix, dir_name));
            
            let new_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };
            
            build_tree(subdir, dir_structure, result, &new_prefix, is_last, processed_dirs);
        }
    }
    
    let mut processed_dirs = Vec::new();
    build_tree(&common_root_str, &dir_structure, &mut result, "", true, &mut processed_dirs);
    
    result.push_str("</file_map>\n\n");
    result
}

/// Compresses text to reduce tokens while maintaining readability for LLMs
fn compress_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_comment = false;
    let mut in_string = false;
    let mut prev_char = ' ';
    let mut consecutive_spaces = 0;
    
    // Common words to abbreviate
    let replacements = [
        ("function", "fn"),
        ("return", "ret"),
        ("const ", "c "),
        ("let ", "l "),
        ("var ", "v "),
        ("import ", "imp "),
        ("export ", "exp "),
        ("interface ", "iface "),
        ("implements ", "impl "),
        ("extends ", "ext "),
        ("public ", "pub "),
        ("private ", "priv "),
        ("protected ", "prot "),
        ("static ", "stat "),
        ("class ", "cls "),
        ("struct ", "st "),
        ("string", "str"),
        ("number", "num"),
        ("boolean", "bool"),
    ];
    
    // First pass: apply word replacements
    let mut processed = text.to_string();
    for (from, to) in replacements.iter() {
        processed = processed.replace(from, to);
    }
    
    // Second pass: remove unnecessary whitespace and comments
    for c in processed.chars() {
        // Handle comment detection
        if prev_char == '/' && c == '/' {
            in_comment = true;
            // Remove the last added '/'
            if !result.is_empty() {
                result.pop();
            }
            continue;
        }
        
        // End comment at newline
        if in_comment && c == '\n' {
            in_comment = false;
            result.push(c);
            prev_char = c;
            consecutive_spaces = 0;
            continue;
        }
        
        // Skip comment content
        if in_comment {
            continue;
        }
        
        // Handle string literals
        if c == '"' && prev_char != '\\' {
            in_string = !in_string;
        }
        
        // Preserve content inside strings
        if in_string {
            result.push(c);
            prev_char = c;
            continue;
        }
        
        // Handle whitespace compression
        if c.is_whitespace() {
            consecutive_spaces += 1;
            // Only keep one space or newlines
            if c == '\n' || consecutive_spaces <= 1 {
                result.push(c);
            }
        } else {
            consecutive_spaces = 0;
            result.push(c);
        }
        
        prev_char = c;
    }
    
    result
}

pub fn copy_file_contents_with_compression(
    contents: &[(String, String)], 
    include_filenames: bool,
    use_compression: bool,
    root_folder: &str
) -> Result<(), Box<dyn Error>> {
    let mut concatenated = String::new();
    
    // Add file tree representation if include_filenames is true
    if include_filenames {
        let file_paths: Vec<String> = contents.iter().map(|(path, _)| path.clone()).collect();
        let tree = generate_file_tree(&file_paths);
        concatenated.push_str(&tree);
    }
    
    for (path, content) in contents {
        // Make path relative to root folder
        let relative_path = if path.starts_with(root_folder) {
            if root_folder.ends_with('/') || root_folder.ends_with('\\') {
                path.strip_prefix(root_folder).unwrap_or(path)
            } else {
                path.strip_prefix(&format!("{}/", root_folder))
                    .or_else(|| path.strip_prefix(&format!("{}\\", root_folder)))
                    .unwrap_or(path)
            }
        } else {
            path
        };
        
        // Always include file meta info
        concatenated.push_str(&format!("// ---- File: {} ----\n", relative_path));
        
        // Apply compression if enabled
        if use_compression {
            let compressed = compress_text(content);
            concatenated.push_str(&compressed);
        } else {
            concatenated.push_str(content);
        }
        
        concatenated.push_str("\n\n");
    }
    
    if use_compression {
        concatenated.push_str("\n// Note: This text has been compressed to reduce tokens while maintaining LLM readability.");
    }
    
    copy_to_clipboard(&concatenated)
}

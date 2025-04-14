use tokio::sync::mpsc;
use walkdir::WalkDir;
use glob::Pattern;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

pub struct Scanner;

impl Scanner {
    pub fn new() -> Self {
        Scanner
    }

    pub async fn scan_directory(&self, path: &str, tx_progress: mpsc::Sender<usize>) -> Vec<FileInfo> {
        let mut files = Vec::new();
        let mut count = 0;

        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            count += 1;
            
            // Only send progress updates every 100 files to reduce overhead
            if count % 100 == 0 {
                let _ = tx_progress.send(count).await;
            }
            
            let path = entry.path();
            let metadata = match path.metadata() {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            
            files.push(FileInfo {
                path: path.to_string_lossy().into_owned(),
                is_dir: metadata.is_dir(),
                size: if metadata.is_file() { metadata.len() } else { 0 },
            });
        }
        
        // Send final count
        let _ = tx_progress.send(count).await;
        
        files
    }
    
    pub async fn scan_directory_with_filters(
        &self, 
        path: &str, 
        tx_progress: mpsc::Sender<usize>, 
        exclude_patterns: &Vec<String>,
        include_patterns: &Vec<String>
    ) -> Vec<FileInfo> {
        let mut files = Vec::new();
        let mut count = 0;
        
        // Compile patterns for better performance
        let compiled_exclude_patterns: Vec<Pattern> = exclude_patterns.iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();
            
        let compiled_include_patterns: Vec<Pattern> = include_patterns.iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();

        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            count += 1;
            
            // Only send progress updates every 100 files to reduce overhead
            if count % 100 == 0 {
                let _ = tx_progress.send(count).await;
            }
            
            let path_str = entry.path().to_string_lossy();
            
            // Apply filters
            if self.should_filter_path(&compiled_exclude_patterns, &compiled_include_patterns, &path_str) {
                continue;
            }
            
            let metadata = match entry.path().metadata() {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            
            files.push(FileInfo {
                path: path_str.into_owned(),
                is_dir: metadata.is_dir(),
                size: if metadata.is_file() { metadata.len() } else { 0 },
            });
        }
        
        // Send final count
        let _ = tx_progress.send(count).await;
        
        files
    }
    
    fn should_filter_path(&self, exclude_patterns: &Vec<Pattern>, include_patterns: &Vec<Pattern>, path: &str) -> bool {
        // If there are include patterns, the path must match at least one
        if !include_patterns.is_empty() {
            let matches_include = include_patterns.iter().any(|pattern| pattern.matches(path));
            
            if !matches_include {
                return true; // Filter out if it doesn't match any include pattern
            }
        }
        
        // Check if the path matches any exclude pattern
        exclude_patterns.iter().any(|pattern| pattern.matches(path))
    }
}

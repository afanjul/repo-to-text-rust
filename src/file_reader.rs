use std::io::Result;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};
use std::collections::HashMap;
use tokio::fs;
use std::time::SystemTime;

#[derive(Clone)]
pub struct FileContentReader {
    cache: HashMap<String, (String, SystemTime)>,
}

impl FileContentReader {
    pub fn new() -> Self {
        FileContentReader {
            cache: HashMap::new(),
        }
    }

    pub async fn read_file(&mut self, path: &str) -> Result<String> {
        // Check if file is in cache and hasn't been modified
        if let Some((content, cached_time)) = self.cache.get(path) {
            if let Ok(metadata) = fs::metadata(path).await {
                if let Ok(modified) = metadata.modified() {
                    if modified <= *cached_time {
                        return Ok(content.clone());
                    }
                }
            }
        }

        // Read file if not in cache or modified
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        let mut contents = String::with_capacity(1024 * 1024); // Pre-allocate 1MB
        let mut buffer = [0; 8192]; // 8KB chunks
        let max_size = 10 * 1024 * 1024; // 10MB limit per file
        let mut total_read = 0;

        loop {
            let bytes_read = reader.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            total_read += bytes_read;
            if total_read > max_size {
                contents.push_str("[Content truncated due to size limit]");
                break;
            }
            // Assume UTF-8 encoding for now
            if let Ok(text) = std::str::from_utf8(&buffer[..bytes_read]) {
                contents.push_str(text);
            } else {
                // Fallback for non-UTF-8 content - could be improved
                contents.push_str("[Non-UTF-8 content]");
                break;
            }
        }

        // Update cache with current time
        let modified_time = fs::metadata(path).await
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::now());
        self.cache.insert(path.to_string(), (contents.clone(), modified_time));

        Ok(contents)
    }

    pub async fn read_multiple_files(&self, paths: Vec<&str>) -> Result<Vec<(String, String)>> {
        let mut results = Vec::with_capacity(paths.len());
        let mut tasks = Vec::new();

        for path in paths {
            let path_owned = path.to_string();
            tasks.push(tokio::spawn(async move {
                let mut reader = FileContentReader::new();
                match reader.read_file(&path_owned).await {
                    Ok(content) => (path_owned, content),
                    Err(e) => (path_owned, format!("Error reading file: {}", e)),
                }
            }));
        }

        for task in tasks {
            if let Ok(result) = task.await {
                results.push(result);
            }
        }

        Ok(results)
    }
}

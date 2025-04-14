use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input, Space, vertical_rule};
use iced::{Alignment, Application, Color, Command, Element, Length, Settings, Theme};
use iced::theme::{self, Text};
use iced::keyboard::{self, Key};
use iced::{Event};
use iced::{subscription, event};
use iced::executor;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use rfd::FileDialog;
use tokio::sync::mpsc;
use glob::Pattern;
use tokio::task::yield_now;

use crate::fs_ops::{FileInfo, Scanner};
use crate::file_reader::FileContentReader;
use crate::clipboard_ops;

// Define simple text-based icons
struct Icons;

impl Icons {
    const FOLDER_CLOSED: &'static str = ">";
    const FOLDER_OPEN: &'static str = "v";
    const FILE: &'static str = "-";
    const SEARCH: &'static str = "S";
    const COPY: &'static str = "C";
    const LIGHT: &'static str = "L";
    const DARK: &'static str = "D";
    const AUTO: &'static str = "A";
    const SETTINGS: &'static str = "⚙";
}

#[derive(Debug, Clone)]
pub enum ThemeType {
    Light,
    Dark,
    Auto,
}

pub struct RepoToTextApp {
    current_path: String,
    files: Vec<FileInfo>,
    selected_files: HashSet<usize>,
    selected_folders: HashSet<String>,
    expanded_folders: HashSet<String>,
    last_selected: Option<usize>,
    token_counts: Vec<(usize, usize)>,
    total_tokens: usize,
    scanning: bool,
    progress: f32,
    cmd_pressed: bool,
    shift_pressed: bool,
    search_query: String,
    file_contents: Vec<(String, String)>,
    reading_files: bool,
    copy_status: String,
    file_reader: FileContentReader,
    
    // UI state
    theme: ThemeType,
    include_file_tree: bool,
    use_compression: bool,
    show_filter_settings: bool,
    exclude_patterns: Vec<String>,
    include_patterns: Vec<String>,
    exclude_patterns_input: String,
    include_patterns_input: String,
    sorted_by_tokens: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    PathChanged(String),
    ScanButtonPressed,
    SelectDirectory,
    ScanProgress(usize),
    ScanComplete(Vec<FileInfo>),
    FileSelected(usize),
    KeyboardEvent(Event),
    ReadSelectedFiles,
    FileContentsRead(Vec<(String, String)>),
    CopyToClipboard,
    CopyStatusUpdated(String),
    SelectAllFiles,
    ClearSelection,
    SearchQueryChanged(String),
    ThemeChanged(ThemeType),
    ToggleIncludeFileTree(bool),
    ToggleCompression(bool),
    TokenCountsCalculated(Vec<(usize, usize)>, usize),
    ToggleFolderExpanded(String),
    FolderSelected(String, bool),
    ToggleFilterSettings,
    ExcludePatternsChanged(String),
    IncludePatternsChanged(String),
    ApplyFilters,
    DirectorySelected(String),
    ProgressUpdate(usize),
}

impl Application for RepoToTextApp {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        // Default exclude patterns for common directories
        let default_exclude_patterns = vec![
            "**/node_modules/**".to_string(),
            "**/.git/**".to_string(),
            "**/vendor/**".to_string(),
            "**/target/**".to_string(),
            "**/dist/**".to_string(),
            "**/build/**".to_string(),
            "**/.idea/**".to_string(),
            "**/.vscode/**".to_string(),
            "**/bin/**".to_string(),
            "**/obj/**".to_string(),
            "**/__pycache__/**".to_string(),
            "**/.DS_Store".to_string(),
        ];
        
        let exclude_patterns_input = default_exclude_patterns.join("\n");
        
        // Try to load the last directory from a config file
        let last_dir = std::fs::read_to_string("last_directory.txt").unwrap_or_default();
        
        let app = Self {
            current_path: last_dir.trim().to_string(),
            files: Vec::new(),
            selected_files: HashSet::new(),
            selected_folders: HashSet::new(),
            expanded_folders: HashSet::new(),
            last_selected: None,
            token_counts: Vec::new(),
            total_tokens: 0,
            scanning: false,
            progress: 0.0,
            cmd_pressed: false,
            shift_pressed: false,
            search_query: String::new(),
            file_contents: Vec::new(),
            reading_files: false,
            copy_status: String::new(),
            file_reader: FileContentReader::new(),
            theme: ThemeType::Light,
            include_file_tree: false,
            use_compression: false,
            show_filter_settings: false,
            exclude_patterns: default_exclude_patterns,
            include_patterns: Vec::new(),
            exclude_patterns_input,
            include_patterns_input: String::new(),
            sorted_by_tokens: true,
        };
        
        // If we have a saved directory, scan it on startup
        let command = if !app.current_path.is_empty() {
            Command::perform(
                async { Message::ScanButtonPressed },
                |msg| msg
            )
        } else {
            Command::none()
        };
        
        (app, command)
    }

    fn subscription(&self) -> subscription::Subscription<Message> {
        event::listen().map(Message::KeyboardEvent)
    }

    fn title(&self) -> String {
        String::from("PasteMax")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PathChanged(path) => {
                self.current_path = path;
                Command::none()
            }
            Message::ScanButtonPressed => {
                if !self.scanning && !self.current_path.is_empty() {
                    self.scanning = true;
                    self.progress = 0.0;
                    let path = self.current_path.clone();
                    let exclude_patterns = self.exclude_patterns.clone();
                    let include_patterns = self.include_patterns.clone();
                    Command::perform(
                        async move {
                            let (tx, mut rx) = mpsc::channel(100);
                            let scanner = Scanner::new();
                            let handle = tokio::spawn(async move {
                                scanner.scan_directory_with_filters(&path, tx, &exclude_patterns, &include_patterns).await
                            });
                            let mut _count = 0;
                            while let Some(progress) = rx.recv().await {
                                _count = progress;
                            }
                            let files = handle.await.unwrap_or_default();
                            Message::ScanComplete(files)
                        },
                        |result| result
                    )
                } else {
                    Command::none()
                }
            }
            Message::SelectDirectory => {
                let dialog = FileDialog::new().pick_folder();
                if let Some(path) = dialog {
                    self.current_path = path.to_string_lossy().into_owned();
                    
                    // Save the selected directory to a file for next startup
                    let _ = std::fs::write("last_directory.txt", &self.current_path);
                    
                    return Command::perform(async {}, |_| Message::ScanButtonPressed);
                }
                Command::none()
            }
            Message::ScanProgress(count) => {
                self.progress = count as f32 / 100.0; // Rough estimate
                Command::none()
            }
            Message::ScanComplete(files) => {
                self.scanning = false;
                self.files = files;
                self.progress = 100.0;
                self.selected_files.clear();
                self.last_selected = None;
                
                // Calculate token counts for files
                let files_clone = self.files.clone();
                Command::perform(
                    async move {
                        let mut counts = Vec::new();
                        let mut total = 0;
                        for (index, file) in files_clone.iter().enumerate() {
                            let path = &file.path;
                            if let Ok(content) = tokio::fs::read_to_string(path).await {
                                // Count tokens (words) in the content
                                let count = content.split_whitespace().count();
                                counts.push((index, count));
                                total += count;
                            } else {
                                counts.push((index, 0));
                            }
                        }
                        
                        // Sort by token count (highest first)
                        counts.sort_by(|a, b| b.1.cmp(&a.1));
                        
                        Message::TokenCountsCalculated(counts, total)
                    },
                    |result| result
                )
            }
            Message::TokenCountsCalculated(counts, total) => {
                self.token_counts = counts;
                
                // Calculate total tokens for selected files
                let selected_indices = &self.selected_files;
                let total_selected = self.token_counts.iter()
                    .filter(|(idx, _)| selected_indices.contains(idx))
                    .map(|(_, count)| count)
                    .sum();
                
                self.total_tokens = total_selected;
                Command::none()
            }
            Message::FileSelected(index) => {
                if self.cmd_pressed {
                    // Cmd+Click: Toggle selection for non-contiguous selection
                    if self.selected_files.contains(&index) {
                        self.selected_files.remove(&index);
                    } else {
                        self.selected_files.insert(index);
                    }
                    self.last_selected = Some(index);
                } else if self.shift_pressed {
                    // Shift+Click: Range selection
                    if let Some(last) = self.last_selected {
                        let start = last.min(index);
                        let end = last.max(index);
                        for i in start..=end {
                            self.selected_files.insert(i);
                        }
                    } else {
                        self.selected_files.clear();
                        self.selected_files.insert(index);
                    }
                    self.last_selected = Some(index);
                } else {
                    // Single click: Toggle selection
                    if self.selected_files.contains(&index) {
                        self.selected_files.remove(&index);
                    } else {
                        self.selected_files.insert(index);
                    }
                    self.last_selected = Some(index);
                }
                
                // Recalculate total tokens for selected files
                let selected_indices = &self.selected_files;
                let total = self.token_counts.iter()
                    .filter(|(idx, _)| selected_indices.contains(idx))
                    .map(|(_, count)| count)
                    .sum();
                
                self.total_tokens = total;
                Command::none()
            }
            Message::KeyboardEvent(event) => {
                match event {
                    Event::Keyboard(keyboard::Event::KeyPressed { key, location: _, text: _, .. }) => {
                        match key {
                            Key::Named(keyboard::key::Named::Shift) => {
                                self.shift_pressed = true;
                            }
                            Key::Named(keyboard::key::Named::Control) | Key::Named(keyboard::key::Named::Alt) => {
                                self.cmd_pressed = true;
                            }
                            Key::Character(c) if c == "a" && self.cmd_pressed => {
                                return Command::perform(async { () }, |_| Message::SelectAllFiles);
                            }
                            Key::Character(c) if c == "c" && self.cmd_pressed && !self.selected_files.is_empty() => {
                                return Command::perform(async { () }, |_| Message::CopyToClipboard);
                            }
                            Key::Character(c) if c == "r" && self.cmd_pressed && !self.selected_files.is_empty() => {
                                return Command::perform(async { () }, |_| Message::ReadSelectedFiles);
                            }
                            Key::Named(keyboard::key::Named::Escape) => {
                                return Command::perform(async { () }, |_| Message::ClearSelection);
                            }
                            _ => {}
                        }
                    }
                    Event::Keyboard(keyboard::Event::KeyReleased { key, location: _, .. }) => {
                        match key {
                            Key::Named(keyboard::key::Named::Shift) => {
                                self.shift_pressed = false;
                            }
                            Key::Named(keyboard::key::Named::Control) | Key::Named(keyboard::key::Named::Alt) => {
                                self.cmd_pressed = false;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                Command::none()
            }
            Message::ReadSelectedFiles => {
                if !self.reading_files && !self.selected_files.is_empty() {
                    self.reading_files = true;
                    let selected_paths: Vec<String> = self.selected_files.iter()
                        .filter_map(|&index| self.files.get(index))
                        .map(|file| file.path.clone())
                        .collect();
                    let file_reader = self.file_reader.clone();
                    Command::perform(
                        async move {
                            let paths: Vec<&str> = selected_paths.iter().map(|s| s.as_str()).collect();
                            match file_reader.read_multiple_files(paths).await {
                                Ok(contents) => Message::FileContentsRead(contents),
                                Err(_) => Message::FileContentsRead(vec![]),
                            }
                        },
                        |result| result
                    )
                } else {
                    Command::none()
                }
            }
            Message::FileContentsRead(contents) => {
                self.reading_files = false;
                self.file_contents = contents;
                Command::none()
            }
            Message::CopyToClipboard => {
                if self.selected_files.is_empty() {
                    self.copy_status = "No files selected".to_string();
                    return Command::none();
                }
                
                self.reading_files = true;
                self.copy_status = "Reading files...".to_string();
                
                // Get paths of selected files
                let paths: Vec<String> = self.selected_files.iter()
                    .filter_map(|&index| self.files.get(index))
                    .map(|file| file.path.clone())
                    .collect();
                
                if paths.is_empty() {
                    self.reading_files = false;
                    self.copy_status = "No valid files selected".to_string();
                    return Command::none();
                }
                
                // Read file contents and copy to clipboard
                let file_reader = self.file_reader.clone();
                let include_tree = self.include_file_tree;
                let use_compression = self.use_compression;
                let root_folder = self.current_path.clone();
                
                Command::perform(
                    async move {
                        let paths_str: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
                        match file_reader.read_multiple_files(paths_str).await {
                            Ok(contents) => {
                                // Copy directly to clipboard here with optional compression
                                match clipboard_ops::copy_file_contents_with_compression(&contents, include_tree, use_compression, &root_folder) {
                                    Ok(()) => {
                                        let status = if use_compression {
                                            "Copied to clipboard (compressed)"
                                        } else {
                                            "Copied to clipboard"
                                        };
                                        Message::CopyStatusUpdated(status.to_string())
                                    },
                                    Err(e) => Message::CopyStatusUpdated(format!("Clipboard error: {}", e)),
                                }
                            }
                            Err(e) => Message::CopyStatusUpdated(format!("Error reading files: {}", e)),
                        }
                    },
                    |msg| msg
                )
            }
            Message::CopyStatusUpdated(status) => {
                self.copy_status = status;
                Command::none()
            }
            Message::SelectAllFiles => {
                for index in 0..self.files.len() {
                    self.selected_files.insert(index);
                }
                
                // Recalculate total tokens for selected files
                let selected_indices = &self.selected_files;
                let total = self.token_counts.iter()
                    .filter(|(idx, _)| selected_indices.contains(idx))
                    .map(|(_, count)| count)
                    .sum();
                
                self.total_tokens = total;
                Command::none()
            }
            Message::ClearSelection => {
                self.selected_files.clear();
                self.selected_folders.clear();
                self.last_selected = None;
                self.total_tokens = 0;
                Command::none()
            }
            Message::SearchQueryChanged(query) => {
                self.search_query = query;
                Command::none()
            }
            Message::ThemeChanged(theme) => {
                self.theme = theme;
                Command::none()
            }
            Message::ToggleIncludeFileTree(include) => {
                self.include_file_tree = include;
                Command::none()
            }
            Message::ToggleCompression(include) => {
                self.use_compression = include;
                Command::none()
            }
            Message::ToggleFilterSettings => {
                self.show_filter_settings = !self.show_filter_settings;
                Command::none()
            }
            Message::ExcludePatternsChanged(patterns) => {
                self.exclude_patterns_input = patterns;
                Command::none()
            }
            Message::IncludePatternsChanged(patterns) => {
                self.include_patterns_input = patterns;
                Command::none()
            }
            Message::ApplyFilters => {
                // Parse exclude patterns
                self.exclude_patterns = self.exclude_patterns_input
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.trim().to_string())
                    .collect();
                
                // Parse include patterns
                self.include_patterns = self.include_patterns_input
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.trim().to_string())
                    .collect();
                
                // Rescan with new filters if we have a path
                if !self.current_path.is_empty() {
                    return self.update(Message::ScanButtonPressed);
                }
                
                Command::none()
            }
            Message::ToggleFolderExpanded(folder) => {
                if self.expanded_folders.contains(&folder) {
                    self.expanded_folders.remove(&folder);
                } else {
                    self.expanded_folders.insert(folder);
                }
                Command::none()
            }
            Message::FolderSelected(folder, selected) => {
                if selected {
                    self.selected_folders.insert(folder.clone());
                    
                    // Select all files in this folder
                    for (index, file) in self.files.iter().enumerate() {
                        let path = Path::new(&file.path);
                        let parent = path.parent()
                            .and_then(|p| p.to_str())
                            .unwrap_or("");
                        
                        if parent == folder {
                            self.selected_files.insert(index);
                        }
                    }
                } else {
                    self.selected_folders.remove(&folder);
                    
                    // Deselect all files in this folder
                    let to_remove: Vec<usize> = self.selected_files.iter()
                        .filter(|&&index| {
                            if let Some(file) = self.files.get(index) {
                                let path = Path::new(&file.path);
                                let parent = path.parent()
                                    .and_then(|p| p.to_str())
                                    .unwrap_or("");
                                return parent == folder;
                            }
                            false
                        })
                        .cloned()
                        .collect();
                    
                    for index in to_remove {
                        self.selected_files.remove(&index);
                    }
                }
                
                // Recalculate total tokens for selected files
                let selected_indices = &self.selected_files;
                let total = self.token_counts.iter()
                    .filter(|(idx, _)| selected_indices.contains(idx))
                    .map(|(_, count)| count)
                    .sum();
                
                self.total_tokens = total;
                Command::none()
            }
            Message::DirectorySelected(path) => {
                self.current_path = path.clone();
                self.files.clear();
                self.selected_files.clear();
                self.selected_folders.clear();
                self.expanded_folders.clear();
                self.token_counts.clear();
                self.total_tokens = 0;
                self.scanning = true;
                self.progress = 0.0;
                
                // Save the selected directory to a file for next startup
                let _ = std::fs::write("last_directory.txt", &path);
                
                // Create progress channel
                let (tx_progress, mut rx_progress) = mpsc::channel(100);
                
                // Create scanner
                let scanner = Scanner::new();
                
                // Scan with filters if we have any
                let exclude_patterns = self.exclude_patterns.clone();
                let include_patterns = self.include_patterns.clone();
                let path_clone = path.clone();
                
                let scan_command = if !self.exclude_patterns.is_empty() || !self.include_patterns.is_empty() {
                    Command::perform(
                        async move {
                            scanner.scan_directory_with_filters(&path_clone, tx_progress, &exclude_patterns, &include_patterns).await
                        },
                        Message::ScanComplete
                    )
                } else {
                    Command::perform(
                        async move {
                            scanner.scan_directory(&path, tx_progress).await
                        },
                        Message::ScanComplete
                    )
                };
                
                // Execute scan
                Command::batch(vec![
                    scan_command,
                    Command::perform(
                        async move {
                            let mut count = 0;
                            while let Some(new_count) = rx_progress.recv().await {
                                count = new_count;
                                yield_now().await;
                            }
                            count
                        },
                        Message::ProgressUpdate
                    )
                ])
            }
            Message::ProgressUpdate(count) => {
                self.progress = count as f32 / 100.0; // Rough estimate
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let title_text = text("PasteMax")
            .size(24)
            .style(Text::Color(Color::from_rgb(0.2, 0.2, 0.2)));
        
        // Theme selector
        let theme_selector = row![
            button(text(Icons::LIGHT))
                .on_press(Message::ThemeChanged(ThemeType::Light))
                .style(if matches!(self.theme, ThemeType::Light) {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                }),
            button(text(Icons::DARK))
                .on_press(Message::ThemeChanged(ThemeType::Dark))
                .style(if matches!(self.theme, ThemeType::Dark) {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                }),
            button(text(Icons::AUTO))
                .on_press(Message::ThemeChanged(ThemeType::Auto))
                .style(if matches!(self.theme, ThemeType::Auto) {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                }),
        ]
        .spacing(5);
        
        // Path display and folder selection
        let path_display = text(&self.current_path)
            .size(14);
        
        let select_folder_button = button(text("Select Folder"))
            .on_press(Message::SelectDirectory)
            .style(theme::Button::Primary);
        
        // Header bar
        let header = container(
            row![
                title_text,
                Space::with_width(Length::Fill),
                theme_selector,
                Space::with_width(Length::Fixed(20.0)),
                path_display,
                Space::with_width(Length::Fixed(10.0)),
                select_folder_button,
            ]
            .spacing(10)
            .align_items(Alignment::Center)
        )
        .padding(15)
        .style(theme::Container::Box);
        
        // Left panel - File browser
        let search_input = text_input(
            "Search files...",
            &self.search_query,
        )
        .on_input(Message::SearchQueryChanged)
        .padding(10);
        
        let select_all_button = button(text("Select All"))
            .on_press(Message::SelectAllFiles)
            .style(theme::Button::Secondary);
        
        let deselect_all_button = button(text("Deselect All"))
            .on_press(Message::ClearSelection)
            .style(theme::Button::Secondary);
        
        let filter_button = button(
            text(format!("{} Filters", Icons::SETTINGS))
        )
        .on_press(Message::ToggleFilterSettings)
        .style(theme::Button::Secondary);
        
        let selection_buttons = row![
            select_all_button,
            Space::with_width(Length::Fixed(10.0)),
            deselect_all_button,
            Space::with_width(Length::Fill),
            filter_button,
        ]
        .spacing(5);
        
        // Filter settings panel
        let filter_settings = if self.show_filter_settings {
            let exclude_patterns_input = text_input(
                "Exclude patterns (one per line)",
                &self.exclude_patterns_input,
            )
            .on_input(Message::ExcludePatternsChanged)
            .padding(10);
            
            let include_patterns_input = text_input(
                "Include patterns (one per line)",
                &self.include_patterns_input,
            )
            .on_input(Message::IncludePatternsChanged)
            .padding(10);
            
            let apply_button = button(text("Apply Filters"))
                .on_press(Message::ApplyFilters)
                .style(theme::Button::Primary);
            
            column![
                text("Filter Settings").size(16),
                text("Exclude patterns (e.g. **/node_modules/**)").size(12),
                exclude_patterns_input,
                text("Include patterns (e.g. **/*.js)").size(12),
                include_patterns_input,
                apply_button,
            ]
            .spacing(10)
            .padding(10)
        } else {
            column![]
        };
        
        // Build file list
        let mut file_list = column![];
        
        // Create a hierarchical structure for files and directories
        let mut dir_hierarchy: HashMap<String, Vec<(usize, &FileInfo)>> = HashMap::new();
        let mut top_level_dirs: HashSet<String> = HashSet::new();
        
        // First, organize files by their directory
        for (index, file) in self.files.iter().enumerate() {
            let path = Path::new(&file.path);
            let parent = path.parent()
                .and_then(|p| p.to_str())
                .unwrap_or("");
            
            // Add file to its directory
            dir_hierarchy.entry(parent.to_string())
                .or_insert_with(Vec::new)
                .push((index, file));
            
            // Find the top-level directory for this file
            let current_path = parent;
            let parent_path = Path::new(current_path).parent()
                .and_then(|p| p.to_str())
                .unwrap_or("");
            
            // If the parent path is empty or the same as the current path, this is a top-level directory
            if parent_path.is_empty() || parent_path == current_path || parent_path == self.current_path {
                top_level_dirs.insert(current_path.to_string());
            }
        }
        
        // Filter by search query if needed
        let search_query = self.search_query.to_lowercase();
        
        // Function to recursively render directories and their contents
        fn render_directory<'a>(
            dir_path: &str,
            dir_hierarchy: &HashMap<String, Vec<(usize, &'a FileInfo)>>,
            expanded_folders: &HashSet<String>,
            selected_folders: &HashSet<String>,
            selected_files: &HashSet<usize>,
            token_counts: &Vec<(usize, usize)>,
            search_query: &str,
            depth: usize,
        ) -> Vec<Element<'a, Message>> {
            let mut elements = Vec::new();
            
            // Get the directory name for display
            let dir_name = Path::new(dir_path).file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(if dir_path.is_empty() { "/" } else { dir_path });
            
            // Check if this directory is expanded
            let is_expanded = expanded_folders.contains(dir_path);
            let is_folder_selected = selected_folders.contains(dir_path);
            let folder_icon = if is_expanded { Icons::FOLDER_OPEN } else { Icons::FOLDER_CLOSED };
            
            // Add directory header with folder selection checkbox
            let folder_row = row![
                Space::with_width(Length::Fixed((depth * 15) as f32)), // Indent based on depth
                checkbox(
                    "",
                    is_folder_selected,
                )
                .on_toggle({
                    let dir = dir_path.to_string();
                    move |selected| Message::FolderSelected(dir.clone(), selected)
                }),
                button(
                    text(format!("{} {}", folder_icon, dir_name))
                        .size(14)
                )
                .on_press(Message::ToggleFolderExpanded(dir_path.to_string()))
                .style(theme::Button::Text)
                .width(Length::Fill),
            ]
            .spacing(5)
            .padding(5)
            .align_items(Alignment::Center);
            
            let styled_folder_row = container(folder_row)
                .width(Length::Fill)
                .style(if is_folder_selected {
                    theme::Container::Box
                } else {
                    theme::Container::Transparent
                });
            
            elements.push(styled_folder_row.into());
            
            // If expanded, add files and subdirectories
            if is_expanded {
                // Add files in this directory
                if let Some(files) = dir_hierarchy.get(dir_path) {
                    for &(index, file) in files {
                        // Skip directories as they'll be handled separately
                        if file.is_dir {
                            continue;
                        }
                        
                        let filename = Path::new(&file.path).file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or("");
                        
                        // Skip files that don't match search
                        if !search_query.is_empty() && 
                           !filename.to_lowercase().contains(search_query) &&
                           !dir_path.to_lowercase().contains(search_query) {
                            continue;
                        }
                        
                        let is_selected = selected_files.contains(&index);
                        
                        // Find token count for this file
                        let token_count = token_counts.iter()
                            .find(|(idx, _)| *idx == index)
                            .map(|(_, count)| *count)
                            .unwrap_or(0);
                        
                        let file_row = row![
                            Space::with_width(Length::Fixed(((depth + 1) * 15) as f32)), // Indent files more than their parent folder
                            checkbox(
                                "",
                                is_selected,
                            )
                            .on_toggle(move |_| Message::FileSelected(index)),
                            text(format!("{} {}", Icons::FILE, filename))
                                .size(14),
                            Space::with_width(Length::Fill),
                            text(format!("(~{})", token_count))
                                .size(12)
                                .width(Length::Fixed(80.0)), // Fixed width for token count to prevent cutoff
                        ]
                        .spacing(5)
                        .padding(5)
                        .align_items(Alignment::Center);
                        
                        let styled_row = container(file_row)
                            .width(Length::Fill)
                            .style(if is_selected {
                                theme::Container::Box
                            } else {
                                theme::Container::Transparent
                            });
                        
                        elements.push(styled_row.into());
                    }
                }
                
                // Find subdirectories
                let subdirs: Vec<String> = dir_hierarchy.keys()
                    .filter(|&subdir| {
                        if subdir == dir_path {
                            return false;
                        }
                        
                        let subdir_path = Path::new(subdir);
                        if let Some(parent) = subdir_path.parent() {
                            if let Some(parent_str) = parent.to_str() {
                                return parent_str == dir_path;
                            }
                        }
                        false
                    })
                    .cloned()
                    .collect();
                
                // Sort subdirectories
                let mut sorted_subdirs = subdirs;
                sorted_subdirs.sort();
                
                // Render subdirectories recursively
                for subdir in sorted_subdirs {
                    let subdir_elements = render_directory(
                        &subdir,
                        dir_hierarchy,
                        expanded_folders,
                        selected_folders,
                        selected_files,
                        token_counts,
                        search_query,
                        depth + 1,
                    );
                    elements.extend(subdir_elements);
                }
            }
            
            elements
        }
        
        // Sort top-level directories
        let mut sorted_top_dirs: Vec<String> = top_level_dirs.into_iter().collect();
        sorted_top_dirs.sort();
        
        // Render top-level directories
        for dir in sorted_top_dirs {
            if !search_query.is_empty() {
                // Check if directory or its contents match search
                let dir_matches = dir.to_lowercase().contains(&search_query);
                let contents_match = dir_hierarchy.get(&dir)
                    .map(|files| {
                        files.iter().any(|(_, file)| {
                            let filename = Path::new(&file.path).file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or("");
                            filename.to_lowercase().contains(&search_query)
                        })
                    })
                    .unwrap_or(false);
                
                if !dir_matches && !contents_match {
                    continue;
                }
            }
            
            let elements = render_directory(
                &dir,
                &dir_hierarchy,
                &self.expanded_folders,
                &self.selected_folders,
                &self.selected_files,
                &self.token_counts,
                &search_query,
                0,
            );
            
            for element in elements {
                file_list = file_list.push(element);
            }
        }
        
        let selected_count = self.selected_files.len();
        let token_count = self.total_tokens;
        
        let selected_files_header = row![
            text(format!("Sort: Tokens: High to Low"))
                .size(14),
            Space::with_width(Length::Fill),
            text(format!("{} files | ~{} tokens", selected_count, token_count))
                .size(14),
        ]
        .padding(10);
        
        // Create grid of selected files
        let mut selected_files_grid = column![];
        let mut current_row = row![];
        let mut count = 0;
        
        // Get indices of selected files
        let mut selected_indices: Vec<usize> = self.selected_files.iter().cloned().collect();
        
        // Sort by token count if needed
        if self.sorted_by_tokens {
            selected_indices.sort_by(|&a, &b| {
                let a_count = self.token_counts.iter()
                    .find(|(idx, _)| *idx == a)
                    .map(|(_, count)| *count)
                    .unwrap_or(0);
                
                let b_count = self.token_counts.iter()
                    .find(|(idx, _)| *idx == b)
                    .map(|(_, count)| *count)
                    .unwrap_or(0);
                
                b_count.cmp(&a_count)
            });
        } else {
            selected_indices.sort();
        }
        
        for &index in &selected_indices {
            if let Some(file) = self.files.get(index) {
                let filename = Path::new(&file.path).file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("");
                
                // Find token count for this file
                let token_count = self.token_counts.iter()
                    .find(|(idx, _)| *idx == index)
                    .map(|(_, count)| *count)
                    .unwrap_or(0);
                
                let file_card = container(
                    column![
                        row![
                            text(format!("{} {}", Icons::FILE, filename))
                                .size(14),
                        ],
                        text(format!("~{} tokens", token_count))
                            .size(12),
                    ]
                    .spacing(5)
                    .padding(10)
                )
                .width(Length::Fixed(150.0))
                .height(Length::Fixed(80.0))
                .style(theme::Container::Box);
                
                current_row = current_row.push(file_card);
                count += 1;
                
                // Create a new row after every 3 items
                if count % 3 == 0 {
                    selected_files_grid = selected_files_grid.push(current_row);
                    current_row = row![];
                }
            }
        }
        
        // Add the last row if it has any items
        if count % 3 != 0 {
            selected_files_grid = selected_files_grid.push(current_row);
        }
        
        let include_tree_checkbox = checkbox(
            "Include File Tree",
            self.include_file_tree,
        )
        .on_toggle(Message::ToggleIncludeFileTree);
        
        let use_compression_checkbox = checkbox(
            "Use Compression",
            self.use_compression,
        )
        .on_toggle(Message::ToggleCompression);
        
        let copy_button = button(
            text(format!("COPY ALL SELECTED ({} files)", selected_count))
                .size(16)
        )
        .on_press(Message::CopyToClipboard)
        .width(Length::Fill)
        .style(theme::Button::Primary);
        
        let selected_files_panel = container(
            column![
                text("Selected Files")
                    .size(18),
                selected_files_header,
                scrollable(
                    container(selected_files_grid)
                        .width(Length::Fill)
                        .padding(10)
                )
                .height(Length::Fill),
                include_tree_checkbox,
                use_compression_checkbox,
                copy_button,
            ]
            .spacing(10)
            .padding(10)
        )
        .width(Length::FillPortion(2))
        .height(Length::Fill)
        .style(theme::Container::Box);
        
        // Status bar
        let status_text = if !self.copy_status.is_empty() {
            &self.copy_status
        } else if self.scanning {
            "Scanning repository..."
        } else if self.reading_files {
            "Reading selected files..."
        } else {
            "Ready"
        };
        
        let status_bar = container(
            row![
                text(status_text)
                    .size(14),
            ]
            .padding(10)
        )
        .width(Length::Fill)
        .style(theme::Container::Box);
        
        // Main layout
        let content = column![
            header,
            container(
                row![
                    container(
                        column![
                            text("Files")
                                .size(18),
                            text(&self.current_path)
                                .size(14),
                            search_input,
                            selection_buttons,
                            filter_settings,
                            scrollable(file_list)
                                .height(Length::Fill),
                        ]
                        .spacing(10)
                        .padding(10)
                    )
                    .width(Length::FillPortion(1))
                    .height(Length::Fill)
                    .style(theme::Container::Box),
                    vertical_rule(1),
                    selected_files_panel,
                ]
                .height(Length::Fill)
            )
            .height(Length::Fill),
            status_bar,
        ];
        
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn theme(&self) -> Theme {
        match self.theme {
            ThemeType::Light => Theme::Light,
            ThemeType::Dark => Theme::Dark,
            ThemeType::Auto => Theme::Light, // Default to light, would need system detection
        }
    }
}

pub fn start_ui() -> iced::Result {
    RepoToTextApp::run(Settings::default())
}

// Function to check if a path should be filtered out based on patterns
fn should_filter_path(exclude_patterns: &Vec<String>, include_patterns: &Vec<String>, path: &str) -> bool {
    // If there are include patterns, the path must match at least one
    if !include_patterns.is_empty() {
        let matches_include = include_patterns.iter().any(|pattern| {
            match Pattern::new(pattern) {
                Ok(glob) => glob.matches(path),
                Err(_) => false,
            }
        });
        
        if !matches_include {
            return true; // Filter out if it doesn't match any include pattern
        }
    }
    
    // Check if the path matches any exclude pattern
    exclude_patterns.iter().any(|pattern| {
        match Pattern::new(pattern) {
            Ok(glob) => glob.matches(path),
            Err(_) => false,
        }
    })
}

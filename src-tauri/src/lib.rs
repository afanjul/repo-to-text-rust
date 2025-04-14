use tauri::{
    command,
    AppHandle, Manager, State,
};
use tauri_plugin_dialog::{FileDialogBuilder, DialogExt};
use std::sync::Mutex;

// Importamos los módulos de nuestra aplicación
mod file_reader;
mod fs_ops;
mod clipboard_ops;

// Estructura para mantener el estado de la aplicación
#[derive(Default)]
struct AppState {
    last_directory: Mutex<Option<String>>,
    selected_files: Mutex<Vec<String>>,
}

// Implementación de comandos Tauri
#[command]
fn scan_directory(app_state: State<'_, AppState>, path: String) -> Result<Vec<fs_ops::FileEntry>, String> {
    let entries = fs_ops::scan_directory(&path).map_err(|e| e.to_string())?;
    
    // Actualizamos el último directorio escaneado
    let mut last_dir = app_state.last_directory.lock().unwrap();
    *last_dir = Some(path);
    
    Ok(entries)
}

#[command]
fn get_last_directory(app_state: State<'_, AppState>) -> Option<String> {
    let last_dir = app_state.last_directory.lock().unwrap();
    last_dir.clone()
}

#[command]
fn select_files(app_state: State<'_, AppState>, files: Vec<String>) -> Result<(), String> {
    let mut selected = app_state.selected_files.lock().unwrap();
    *selected = files;
    Ok(())
}

#[command]
fn get_selected_files(app_state: State<'_, AppState>) -> Vec<String> {
    let selected = app_state.selected_files.lock().unwrap();
    selected.clone()
}

#[command]
async fn copy_files_content(
    app_state: State<'_, AppState>, 
    app_handle: AppHandle, 
    include_tree: bool,
    root_directory: String
) -> Result<String, String> {
    let selected = app_state.selected_files.lock().unwrap();
    
    if selected.is_empty() {
        return Err("No hay archivos seleccionados".to_string());
    }
    
    // Leer el contenido de los archivos, pasando el directorio raíz
    let content = file_reader::read_files(&selected, &root_directory).map_err(|e| e.to_string())?;
    
    // Generar el contenido final, posiblemente con el árbol de archivos
    let final_content = if include_tree {
        // Generar el árbol de archivos y combinarlo con el contenido, pasando el directorio raíz
        let tree = fs_ops::generate_file_tree(&selected, &root_directory)?;
        format!("{}\n\n{}", tree, content)
    } else {
        content
    };
    
    // Usar el plugin de clipboard de Tauri
    let clipboard = app_handle.state::<tauri_plugin_clipboard::Clipboard>();
    clipboard.write_text(final_content).map_err(|e| e.to_string())?;
    
    Ok(format!("Contenido de {} archivos copiado al portapapeles", selected.len()))
}

#[command]
fn open_directory_dialog() -> Result<(), String> {
    // Este comando ya no lo usaremos directamente desde JS para abrir el diálogo.
    // Lo dejamos por si acaso, pero la lógica principal se moverá.
    Ok(())
}

#[command]
async fn trigger_directory_dialog(app_handle: AppHandle) -> Result<Option<String>, String> {
    let handle_clone = app_handle.clone(); // Clonamos para el closure
    // Usamos FileDialogBuilder directamente desde Rust
    // ¡IMPORTANTE! Tauri espera un closure aquí. Usamos tokio::sync::oneshot 
    // para esperar el resultado de forma asíncrona y poder devolverlo.
    let (tx, rx) = tokio::sync::oneshot::channel();

    FileDialogBuilder::new(handle_clone.dialog().clone())
        .pick_folder(move |folder_path| {
            let _ = tx.send(folder_path.map(|p| p.to_string()));
        });

    // Esperamos el resultado del diálogo
    match rx.await {
        Ok(path_option) => Ok(path_option),
        Err(_) => Err("El canal del diálogo se cerró inesperadamente".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Inicializamos el estado de la aplicación
            app.manage(AppState::default());
            Ok(())
        })
        .plugin(tauri_plugin_clipboard::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build())
        .invoke_handler(tauri::generate_handler![
            scan_directory,
            get_last_directory,
            select_files,
            get_selected_files,
            copy_files_content,
            open_directory_dialog,
            trigger_directory_dialog, // <-- Añadir el nuevo comando
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

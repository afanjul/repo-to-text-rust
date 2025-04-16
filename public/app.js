// Estado de la aplicación (global)
let currentDirectory = ''; // Directorio actual en el explorador
let rootDirectory = '';    // Directorio raíz seleccionado con "Abrir Directorio"
let selectedFiles = [];

// Elementos del DOM (serán asignados dentro de DOMContentLoaded)
let openDirBtn, copyBtn, currentPath, fileList, selectedList, status, includeTreeCheckbox;

// APIs de Tauri (serán asignadas dentro de DOMContentLoaded)
let invoke, writeText, open;

document.addEventListener('DOMContentLoaded', async () => {
    // **** DEBUG: Verificar si __TAURI__ existe ****
    console.log("DOMContentLoaded event fired.");
    console.log("window.__TAURI__ object:", window.__TAURI__);
    // **** FIN DEBUG ****
    
    // Asignar APIs de Tauri ahora que el DOM está listo y Tauri debería estar inyectado
    // Añadimos una comprobación para evitar el error si __TAURI__ sigue sin estar definido
    // **** DEBUG: Check individual APIs ****
    let apiCheckFailed = false;
    if (!window.__TAURI__) {
        console.error("Error Crítico: window.__TAURI__ no está definido.");
        apiCheckFailed = true;
    } else {
        if (!window.__TAURI__.core) {
            console.error("Error: window.__TAURI__.core no está definido.");
            apiCheckFailed = true;
        }
        if (!window.__TAURI__.clipboard) {
            console.error("Error: window.__TAURI__.clipboard no está definido.");
            // Nota: Podrías continuar sin clipboard si no es crítico inicialmente
            // apiCheckFailed = true; 
        }
        if (!window.__TAURI__.dialog) {
            console.error("Error: window.__TAURI__.dialog no está definido.");
            apiCheckFailed = true;
        }
    }
    // **** FIN DEBUG ****
    
    if (apiCheckFailed) {
        console.error("Fallo en la carga de APIs de Tauri! Asegúrate que la aplicación se ejecuta con 'cargo tauri dev' o 'cargo tauri build' y que los plugins están correctamente configurados.");
        updateStatus("Error crítico: No se pudieron cargar las APIs de Tauri.");
        return; // Detener ejecución si las APIs críticas no están
    }
    
    ({ invoke } = window.__TAURI__.core);
    // Asignar clipboard y dialog solo si existen (manejo defensivo)
    if (window.__TAURI__.clipboard) {
        ({ writeText } = window.__TAURI__.clipboard);
    }
    if (window.__TAURI__.dialog) {
        ({ open } = window.__TAURI__.dialog);
    }

    // Asignar elementos del DOM
    openDirBtn = document.getElementById('openDirBtn');
    copyBtn = document.getElementById('copyBtn');
    currentPath = document.getElementById('currentPath');
    fileList = document.getElementById('fileList');
    selectedList = document.getElementById('selectedList');
    status = document.getElementById('status');
    includeTreeCheckbox = document.getElementById('includeTreeCheckbox');

    // --- Inicialización --- 
    try {
        // Intentamos cargar el último directorio utilizado
        const lastDir = await invoke('get_last_directory');
        if (lastDir) {
            currentDirectory = lastDir;
            currentPath.textContent = lastDir;
            await loadDirectoryContents(lastDir);
        } else {
            updateStatus('Selecciona un directorio para empezar.');
        }
    } catch (error) {
        updateStatus(`Error al inicializar: ${error}`);
        console.error("Initialization Error:", error);
    }
    
    // --- Event Listeners --- 
    openDirBtn.addEventListener('click', async () => {
        console.log("Open Directory button clicked");
        updateStatus("Abriendo diálogo de directorio...");
        try {
            // Llamamos al comando Rust para seleccionar un directorio
            const selectedPath = await invoke('trigger_directory_dialog');
            
            if (selectedPath) {
                console.log("Root directory selected:", selectedPath);
                // Actualizar el directorio raíz y el directorio actual
                rootDirectory = selectedPath;
                currentDirectory = selectedPath;
                
                // Mostrar el directorio raíz en el path-display
                currentPath.textContent = rootDirectory;
                
                // Cargar el contenido del directorio
                await loadDirectoryContents(selectedPath);
                
                // Limpiar archivos seleccionados al cambiar de directorio raíz
                selectedFiles = [];
                selectedList.innerHTML = '';
                copyBtn.disabled = true;
                
                // Actualizar el estado en el backend
                await invoke('select_files', { files: selectedFiles });
                
                updateStatus('Directorio raíz cargado correctamente');
            } else {
                updateStatus('Selección de directorio cancelada');
            }
        } catch (error) {
            updateStatus(`Error al abrir el directorio: ${error}`);
            console.error("Open Directory Error:", error);
        }
    });

    copyBtn.addEventListener('click', async () => {
        if (selectedFiles.length === 0) {
            updateStatus('No hay archivos seleccionados para copiar');
            return;
        }
        
        try {
            updateStatus('Copiando contenido al portapapeles...');
            // Enviar la opción de incluir árbol y el directorio raíz
            const includeTree = includeTreeCheckbox.checked;
            const result = await invoke('copy_files_content', { 
                includeTree,
                rootDirectory  // Usar el directorio raíz global
            }); 
            updateStatus(result);
        } catch (error) {
            updateStatus(`Error al copiar al portapapeles: ${error}`);
            console.error("Copy Content Error:", error);
        }
    });
});

// --- Funciones (permanecen fuera del listener) --- 
async function loadDirectoryContents(path) {
    // Primero, verifica si invoke está definido
    if (!invoke) {
        updateStatus("Error: La API de Tauri no está lista.");
        console.error("loadDirectoryContents called before invoke was defined.");
        return;
    }
    try {
        updateStatus('Cargando contenido del directorio...');
        const entries = await invoke('scan_directory', { path });
        
        // Limpiar la lista de archivos
        fileList.innerHTML = '';
        
        // Mostrar el botón para subir un nivel solo si no estamos en el directorio raíz
        if (path !== rootDirectory && path.includes('/')) {
            const upItem = document.createElement('div');
            upItem.className = 'file-item navigation-item';
            upItem.innerHTML = `
                <div class="file-icon">↑</div>
                <div class="file-name">.. (Volver al directorio padre)</div>
            `;
            upItem.addEventListener('click', async () => {
                const pathParts = path.split('/').filter(part => part.length > 0);
                const parentPath = '/' + pathParts.slice(0, -1).join('/') || '/';
                
                // Solo actualizar el directorio actual, no el raíz
                currentDirectory = parentPath;
                
                // No actualizar el path-display, que siempre muestra el directorio raíz
                
                await loadDirectoryContents(parentPath);
            });
            fileList.appendChild(upItem);
        }
        
        // Mostrar los archivos y directorios
        entries.forEach(entry => {
            const item = document.createElement('div');
            item.className = 'file-item';
            item.dataset.filePath = entry.path; // Guardar ruta completa para referencia
            item.dataset.isDir = entry.is_dir;
            
            // Verificar si el archivo ya está seleccionado
            if (!entry.is_dir && selectedFiles.includes(entry.path)) {
                item.classList.add('selected');
            }
            
            const icon = entry.is_dir ? '📁' : '📄';
            const size = entry.is_dir ? '' : formatFileSize(entry.size); // No mostrar tamaño para directorios
            
            item.innerHTML = `
                <div class="file-icon">${icon}</div>
                <div class="file-name">${entry.name}</div>
                <div class="file-size">${size}</div>
            `;
            
            item.addEventListener('click', async () => {
                if (entry.is_dir) {
                    currentDirectory = entry.path;
                    await loadDirectoryContents(entry.path);
                } else {
                    toggleFileSelection(entry);
                }
            });
            
            fileList.appendChild(item);
        });
        
        updateStatus(`${entries.length} elementos encontrados`);
    } catch (error) {
        updateStatus(`Error al cargar el directorio: ${error}`);
        console.error("Load Directory Contents Error:", error);
    }
}

function toggleFileSelection(fileEntry) {
     // Primero, verifica si invoke está definido
    if (!invoke) {
        updateStatus("Error: La API de Tauri no está lista.");
        console.error("toggleFileSelection called before invoke was defined.");
        return;
    }
    const filePath = fileEntry.path;
    const fileName = fileEntry.name;
    const index = selectedFiles.findIndex(path => path === filePath);
    
    if (index === -1) {
        // Añadir a seleccionados
        selectedFiles.push(filePath);
        
        // Añadir a la lista visual de seleccionados
        const item = document.createElement('div');
        item.className = 'selected-item';
        item.dataset.path = filePath; // Usar filePath guardado
        
        item.innerHTML = `
            <div class="file-icon">📄</div>
            <div class="file-name">${fileName}</div> 
            <button class="remove-btn">×</button>
        `;
        
        item.querySelector('.remove-btn').addEventListener('click', (e) => {
            e.stopPropagation();
            removeFileSelection(filePath);
        });
        
        selectedList.appendChild(item);
    } else {
        // Quitar de seleccionados
        removeFileSelection(filePath);
    }
    
    updateFileListSelection(); // Actualiza la lista principal
    copyBtn.disabled = selectedFiles.length === 0;
    
    // Actualizar el estado en el backend
    invoke('select_files', { files: selectedFiles }).catch(err => {
        updateStatus(`Error al actualizar selección: ${err}`);
        console.error("Update Selection Error:", err);
    });
    
    updateStatus(`${selectedFiles.length} archivos seleccionados`);
}

function removeFileSelection(path) {
     // Primero, verifica si invoke está definido
    if (!invoke) {
        updateStatus("Error: La API de Tauri no está lista.");
        console.error("removeFileSelection called before invoke was defined.");
        return;
    }
    const index = selectedFiles.findIndex(p => p === path);
    if (index !== -1) {
        selectedFiles.splice(index, 1);
        
        const item = selectedList.querySelector(`[data-path="${CSS.escape(path)}"]`); // Usar CSS.escape para rutas con caracteres especiales
        if (item) {
            selectedList.removeChild(item);
        }
        
        updateFileListSelection(); // Actualiza la lista principal
        copyBtn.disabled = selectedFiles.length === 0;
        
        invoke('select_files', { files: selectedFiles }).catch(err => {
             updateStatus(`Error al actualizar selección: ${err}`);
             console.error("Update Selection Error:", err);
        });
    }
}

function updateFileListSelection() {
    const fileItems = fileList.querySelectorAll('.file-item[data-file-path]'); // Seleccionar solo items de archivo/directorio, no de navegación
    fileItems.forEach(item => {
        const filePath = item.dataset.filePath;
        if (filePath && selectedFiles.includes(filePath)) {
            item.classList.add('selected');
        } else {
            item.classList.remove('selected');
        }
    });
}

function updateStatus(message) {
    // Asegurarse de que 'status' existe antes de intentar modificarlo
    if (status) {
        status.textContent = message;
    } else {
        console.log("Status Update:", message); // Fallback si el elemento DOM no está listo
    }
}

function formatFileSize(bytes) {
    if (bytes === undefined || bytes === null || isNaN(bytes)) return ''; // Manejar casos indefinidos o no numéricos
    if (bytes < 0) return ''; // No mostrar tamaño para directorios/errores
    if (bytes === 0) return '0 B';
    
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    
    // Asegurarse de que i esté dentro de los límites de units
    const unitIndex = Math.max(0, Math.min(i, units.length - 1));
    
    return `${(bytes / Math.pow(1024, unitIndex)).toFixed(1)} ${units[unitIndex]}`;
}

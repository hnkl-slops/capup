use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_store::StoreExt;

const STORE: &str = "settings.json";
const KEY: &str = "saveDir";

/// Returns the folder recordings are saved to.
/// It is the folder the user chose, or the user's video folder until they choose one.
pub fn get<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let chosen = app
        .store(STORE)
        .map_err(|e| e.to_string())?
        .get(KEY)
        .and_then(|value| value.as_str().map(PathBuf::from));
    let Some(dir) = chosen else {
        return app.path().video_dir().map_err(|e| e.to_string());
    };
    if !dir.is_dir() {
        return Err(format!(
            "The save folder does not exist: {}. Choose another one from the tray menu.",
            dir.display()
        ));
    }
    Ok(dir)
}

/// Lets the user pick the folder recordings are saved to, and remembers it.
pub fn choose<R: Runtime>(app: &AppHandle<R>) {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Choose where to save recordings");
    if let Ok(current) = get(app) {
        dialog = dialog.set_directory(current);
    }

    let app = app.clone();
    dialog.pick_folder(move |folder| {
        let Some(Ok(dir)) = folder.map(|folder| folder.into_path()) else {
            return;
        };
        if let Ok(store) = app.store(STORE) {
            store.set(KEY, dir.to_string_lossy());
            let _ = store.save();
        }
    });
}

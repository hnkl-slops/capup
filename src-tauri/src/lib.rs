mod on_screen;
mod recorder;
mod region;
mod save_dir;

use std::sync::Mutex;

use tauri::{
    App, AppHandle, Manager, RunEvent, State, WebviewWindow, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use recorder::Recorder;
use region::Region;

#[derive(Default)]
struct RecorderState(Mutex<Option<Recorder>>);

/// Starts recording what is behind the window into `file_name` in the save folder.
/// Returns the path of the mp4 file.
#[tauri::command]
fn start_recording(
    window: WebviewWindow,
    state: State<RecorderState>,
    file_name: String,
) -> Result<String, String> {
    let mut recorder = state.0.lock().unwrap();
    if recorder.is_some() {
        return Err("Already recording".into());
    }

    let output = save_dir::get(window.app_handle())?.join(file_name);
    *recorder = Some(Recorder::start(Region::of_window(&window)?, &output)?);

    // The recorded area is fixed at start, and clicks must reach the apps behind the window.
    window.set_resizable(false).map_err(|e| e.to_string())?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into_owned())
}

/// Stops the running recording and finalizes its mp4 file.
#[tauri::command]
fn stop_recording(window: WebviewWindow, state: State<RecorderState>) -> Result<(), String> {
    let recorder = state.0.lock().unwrap().take().ok_or("Not recording")?;
    window.set_resizable(true).map_err(|e| e.to_string())?;
    window
        .set_ignore_cursor_events(false)
        .map_err(|e| e.to_string())?;
    recorder.finish()
}

/// Puts the app in the menu bar or system tray, where a click opens the window.
fn setup_tray(app: &mut App) -> tauri::Result<()> {
    // The app lives in the menu bar, so it has no Dock icon.
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let choose_save_dir =
        MenuItem::with_id(app, "save_dir", "Choose Save Folder…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit capup", true, None::<&str>)?;
    TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("the bundle has an icon"),
        )
        .tooltip("capup")
        .menu(&Menu::with_items(app, &[&choose_save_dir, &quit])?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "save_dir" => save_dir::choose(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Makes the window come to the desktop the user is on when it is shown, instead of staying on the one it was opened on.
#[cfg(target_os = "macos")]
fn follow_active_space(window: &WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    let ns_window = unsafe { &*(window.ns_window()? as *const NSWindow) };
    ns_window.setCollectionBehavior(
        ns_window.collectionBehavior() | NSWindowCollectionBehavior::MoveToActiveSpace,
    );
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(RecorderState::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            follow_active_space(
                &app.get_webview_window("main")
                    .expect("the main window exists"),
            )?;
            Ok(setup_tray(app)?)
        })
        .on_window_event(|window, event| match event {
            // Closing only hides the window so that the app stays in the tray.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                let _ = on_screen::keep_on_screen(window);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![start_recording, stop_recording])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // An mp4 file is unplayable unless the encoder gets to finalize it.
            if let RunEvent::Exit = event
                && let Some(recorder) = app.state::<RecorderState>().0.lock().unwrap().take()
            {
                let _ = recorder.finish();
            }
        });
}

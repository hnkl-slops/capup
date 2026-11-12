use tauri::WebviewWindow;

use crate::recorder::Target;

/// The area of a display covered by the app window, in physical pixels from the display's top-left corner.
pub struct Region {
    pub target: Target,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    /// Measures the area of the display that the window currently covers.
    /// Must be called on the main thread.
    pub fn of_window(window: &WebviewWindow) -> Result<Self, String> {
        let monitor = window
            .current_monitor()
            .map_err(|e| e.to_string())?
            .ok_or("The window is not on any display")?;
        let position = window.inner_position().map_err(|e| e.to_string())?;
        let size = window.inner_size().map_err(|e| e.to_string())?;

        let left = (position.x - monitor.position().x).max(0);
        let top = (position.y - monitor.position().y).max(0);
        let right = (position.x - monitor.position().x + size.width as i32)
            .min(monitor.size().width as i32);
        let bottom = (position.y - monitor.position().y + size.height as i32)
            .min(monitor.size().height as i32);

        let width = even(right - left);
        let height = even(bottom - top);
        if width == 0 || height == 0 {
            return Err("The window is outside of its display".into());
        }

        Ok(Self {
            target: Target::of_window(window)?,
            x: left as u32,
            y: top as u32,
            width,
            height,
        })
    }
}

/// Rounds down to an even number because H.264 in yuv420 needs even dimensions.
fn even(value: i32) -> u32 {
    value.max(0) as u32 & !1
}

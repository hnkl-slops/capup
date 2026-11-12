use tauri::{PhysicalPosition, PhysicalSize, Runtime, Window};

/// Moves and shrinks the window back into the work area of its display.
/// Only the part of the window on a display can be recorded, and the menu bar or taskbar would cover the controls.
pub fn keep_on_screen<R: Runtime>(window: &Window<R>) -> tauri::Result<()> {
    // A window entirely off its display may have no current display.
    let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) else {
        return Ok(());
    };
    let area = monitor.work_area();
    let position = window.outer_position()?;
    let size = window.outer_size()?;

    let fitted = PhysicalSize::new(
        size.width.min(area.size.width),
        size.height.min(area.size.height),
    );
    let placed = PhysicalPosition::new(
        position.x.clamp(
            area.position.x,
            area.position.x + area.size.width as i32 - fitted.width as i32,
        ),
        position.y.clamp(
            area.position.y,
            area.position.y + area.size.height as i32 - fitted.height as i32,
        ),
    );

    if fitted != size {
        window.set_size(fitted)?;
    }
    if placed != position {
        window.set_position(placed)?;
    }
    Ok(())
}

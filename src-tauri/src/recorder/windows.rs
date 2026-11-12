use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, Sender},
};

use tauri::WebviewWindow;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow},
};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    encoder::{
        AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
        VideoSettingsSubType,
    },
    frame::Frame,
    graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl},
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};

use crate::region::Region;

const FPS: u32 = 30;

/// The display to capture.
/// The app window is left out by its content protection instead of by the capture itself.
pub struct Target {
    monitor: isize,
}

impl Target {
    /// Looks up the display the window is on.
    pub fn of_window(window: &WebviewWindow) -> Result<Self, String> {
        // Rebuilding the handle from its raw pointer keeps this independent of the `windows` version tauri uses.
        let hwnd = HWND(window.hwnd().map_err(|e| e.to_string())?.0);
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        Ok(Self {
            monitor: monitor.0 as isize,
        })
    }
}

/// Records the region until `stop` receives a message or is disconnected.
/// Sends to `ready` once the capture is running.
pub fn record(
    region: Region,
    output: &Path,
    ready: Sender<()>,
    stop: Receiver<()>,
) -> Result<(), String> {
    let border = if GraphicsCaptureApi::is_border_settings_supported().unwrap_or(false) {
        DrawBorderSettings::WithoutBorder
    } else {
        DrawBorderSettings::Default
    };
    let settings = Settings::new(
        Monitor::from_raw_hmonitor(region.target.monitor as *mut _),
        CursorCaptureSettings::Default,
        border,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        Flags {
            region,
            output: output.to_path_buf(),
        },
    );
    let control = Handler::start_free_threaded(settings).map_err(|e| e.to_string())?;

    let _ = ready.send(());
    let _ = stop.recv();

    // The encoder is taken out first so that no frame is sent to it while it finalizes the file.
    let encoder = control.callback().lock().encoder.take();
    control.stop().map_err(|e| e.to_string())?;
    encoder
        .ok_or("The encoder was already finished")?
        .finish()
        .map_err(|e| e.to_string())
}

struct Flags {
    region: Region,
    output: PathBuf,
}

/// Crops each captured frame to the region and sends it to a Media Foundation H.264 encoder.
struct Handler {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    encoder: Option<VideoEncoder>,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = Flags;
    type Error = Box<dyn Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let Flags { region, output } = ctx.flags;
        let encoder = VideoEncoder::new(
            VideoSettingsBuilder::new(region.width, region.height)
                .sub_type(VideoSettingsSubType::H264)
                .frame_rate(FPS),
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            output,
        )?;
        Ok(Self {
            left: region.x,
            top: region.y,
            width: region.width,
            height: region.height,
            encoder: Some(encoder),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let Some(encoder) = self.encoder.as_mut() else {
            return Ok(());
        };
        let timestamp = frame.timestamp()?.Duration;
        let buffer = frame.buffer_crop(
            self.left,
            self.top,
            self.left + self.width,
            self.top + self.height,
        )?;
        let mut unpadded = Vec::new();
        let pixels = buffer.as_nopadding_buffer(&mut unpadded);

        // The encoder takes raw frames with the bottom row first.
        let flipped: Vec<u8> = pixels
            .chunks_exact(self.width as usize * 4)
            .rev()
            .flatten()
            .copied()
            .collect();
        encoder.send_frame_buffer(&flipped, timestamp)?;
        Ok(())
    }
}

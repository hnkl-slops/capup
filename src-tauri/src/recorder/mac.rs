use std::{
    path::Path,
    sync::{
        Mutex,
        mpsc::{Receiver, Sender},
    },
};

use cidre::{
    arc, av, cf, cg, cm, cv, define_obj_type, dispatch, ns, objc,
    sc::{self, StreamOutput as _, StreamOutputImpl},
};
use objc2::{msg_send, runtime::AnyObject};
use objc2_foundation::NSString;
use tauri::{WebviewWindow, async_runtime::block_on};

use crate::region::Region;

const FPS: i32 = 30;

unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

/// The display to capture and the app window to leave out of it.
pub struct Target {
    display_id: u32,
    window_id: u32,
    scale_factor: f64,
}

impl Target {
    /// Looks up the display the window is on. Must be called on the main thread.
    pub fn of_window(window: &WebviewWindow) -> Result<Self, String> {
        let scale_factor = window.scale_factor().map_err(|e| e.to_string())?;
        let ns_window = window.ns_window().map_err(|e| e.to_string())? as *mut AnyObject;
        unsafe {
            let window_number: isize = msg_send![ns_window, windowNumber];
            let screen: *mut AnyObject = msg_send![ns_window, screen];
            if screen.is_null() {
                return Err("The window is not on any display".into());
            }
            let description: *mut AnyObject = msg_send![screen, deviceDescription];
            let screen_number: *mut AnyObject =
                msg_send![description, objectForKey: &*NSString::from_str("NSScreenNumber")];
            Ok(Self {
                display_id: msg_send![screen_number, unsignedIntValue],
                window_id: window_number as u32,
                scale_factor,
            })
        }
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
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        unsafe { CGRequestScreenCaptureAccess() };
        return Err(
            "Screen recording permission is required. Grant it in System Settings, then restart capup."
                .into(),
        );
    }

    let content = block_on(sc::ShareableContent::current()).map_err(|e| describe(&e))?;
    let displays = content.displays();
    let display = displays
        .iter()
        .find(|display| display.display_id().0 == region.target.display_id)
        .ok_or("The display of the window was not found")?;
    let windows = content.windows();
    let own_windows: Vec<_> = windows
        .iter()
        .filter(|window| window.id() == region.target.window_id)
        .collect();
    let filter = sc::ContentFilter::with_display_excluding_windows(
        display,
        &ns::Array::from_slice(&own_windows),
    );

    let stream_output = StreamOutput::with(StreamOutputInner {
        encoder: Mutex::new(Some(Encoder::new(output, region.width, region.height)?)),
    });
    let stream = sc::Stream::new(&filter, &stream_config(&region));
    let queue = dispatch::Queue::serial_with_ar_pool();
    let started = stream
        .add_stream_output(stream_output.as_ref(), sc::OutputType::Screen, Some(&queue))
        .map_err(|e| describe(&e))
        .and_then(|()| block_on(stream.start()).map_err(|e| describe(&e)));
    if let Err(error) = started {
        // Finishing before any frame arrived discards the unplayable file.
        let _ = stream_output.take_encoder().finish();
        return Err(error);
    }

    let _ = ready.send(());
    let _ = stop.recv();

    let stopped = block_on(stream.stop()).map_err(|e| describe(&e));
    stream_output.take_encoder().finish()?;
    stopped
}

fn stream_config(region: &Region) -> arc::R<sc::StreamCfg> {
    // The source rect is in points, while the output size is in pixels.
    let scale = region.target.scale_factor;
    let mut config = sc::StreamCfg::new();
    config.set_src_rect(cg::Rect {
        origin: cg::Point {
            x: f64::from(region.x) / scale,
            y: f64::from(region.y) / scale,
        },
        size: cg::Size {
            width: f64::from(region.width) / scale,
            height: f64::from(region.height) / scale,
        },
    });
    config.set_width(region.width as usize);
    config.set_height(region.height as usize);
    // The hardware H.264 encoder takes this format without a conversion.
    config.set_pixel_format(cv::PixelFormat::_420V);
    config.set_shows_cursor(true);
    config.set_minimum_frame_interval(cm::Time {
        value: 1,
        scale: FPS,
        epoch: 0,
        flags: cm::TimeFlags::VALID,
    });
    config
}

fn describe(error: &ns::Error) -> String {
    error.localized_description().to_string()
}

/// Writes captured frames into an H.264 mp4 file with AVAssetWriter.
struct Encoder {
    writer: arc::R<av::AssetWriter>,
    input: arc::R<av::AssetWriterInput>,
    started: bool,
}

impl Encoder {
    fn new(output: &Path, width: u32, height: u32) -> Result<Self, String> {
        let url = cf::Url::with_path(output, false).ok_or("The output path is invalid")?;
        let mut writer = av::AssetWriter::with_url_and_file_type(url.as_ns(), av::FileType::mp4())
            .map_err(describe)?;

        let width = ns::Number::with_u32(width);
        let height = ns::Number::with_u32(height);
        let settings = ns::Dictionary::with_keys_values(
            &[
                av::video::settings_keys::codec(),
                av::video::settings_keys::width(),
                av::video::settings_keys::height(),
            ],
            &[
                av::VideoCodec::h264().as_id_ref(),
                width.as_id_ref(),
                height.as_id_ref(),
            ],
        );
        let mut input = av::AssetWriterInput::with_media_type_and_output_settings(
            av::MediaType::video(),
            Some(&settings),
        )
        .map_err(|e| format!("{e:?}"))?;
        input.set_expects_media_data_in_real_time(true);
        writer.add_input(&input).map_err(|e| format!("{e:?}"))?;

        if !writer.start_writing() {
            return Err(writer_error(&writer));
        }
        Ok(Self {
            writer,
            input,
            started: false,
        })
    }

    fn append(&mut self, sample: &cm::SampleBuf) {
        // Frames are dropped rather than queued when the encoder is busy, as a live recording must not lag behind.
        if !is_complete_frame(sample) || !self.input.is_ready_for_more_media_data() {
            return;
        }
        if !self.started {
            self.writer.start_session_at_src_time(sample.pts());
            self.started = true;
        }
        let _ = self.input.append_sample_buf(sample);
    }

    fn finish(mut self) -> Result<(), String> {
        if !self.started {
            self.writer.cancel_writing();
            return Err("No frame was captured".into());
        }
        // Frames only arrive when the screen changes, so the video is extended to the stop time explicitly.
        self.writer
            .end_session_at_src_time(cm::Clock::host_time_clock().time());
        self.input.mark_as_finished();
        self.writer.finish_writing();
        if self.writer.status() != av::AssetWriterStatus::Completed {
            return Err(writer_error(&self.writer));
        }
        Ok(())
    }
}

fn writer_error(writer: &av::AssetWriter) -> String {
    writer
        .error()
        .map(|e| describe(&e))
        .unwrap_or_else(|| "Failed to write the video".into())
}

/// ScreenCaptureKit also delivers samples without an image, such as while the screen is idle.
fn is_complete_frame(sample: &cm::SampleBuf) -> bool {
    let status = sample
        .attaches(false)
        .and_then(|attachments| attachments.iter().next())
        .and_then(|info| info.get(sc::FrameInfo::status().as_cf()))
        .and_then(|status| status.as_number().to_i32());
    status == Some(sc::FrameStatus::Complete as i32)
}

#[repr(C)]
struct StreamOutputInner {
    // Shared between the capture queue and the recording thread.
    encoder: Mutex<Option<Encoder>>,
}

define_obj_type!(
    StreamOutput + StreamOutputImpl,
    StreamOutputInner,
    CAPUP_STREAM_OUTPUT
);

impl StreamOutput {
    fn take_encoder(&self) -> Encoder {
        self.inner()
            .encoder
            .lock()
            .unwrap()
            .take()
            .expect("the encoder is taken only once")
    }
}

impl sc::stream::Output for StreamOutput {}

#[objc::add_methods]
impl StreamOutputImpl for StreamOutput {
    extern "C" fn impl_stream_did_output_sample_buf(
        &mut self,
        _cmd: Option<&objc::Sel>,
        _stream: &sc::Stream,
        sample_buf: &mut cm::SampleBuf,
        _kind: sc::OutputType,
    ) {
        if let Some(encoder) = self.inner().encoder.lock().unwrap().as_mut() {
            encoder.append(sample_buf);
        }
    }
}

//! Gstreamer support.
//!
//! This file is licensed under the Unlicense.

use std::sync::LazyLock;

use anyhow::{anyhow, Result};
use derive_more::derive::{Display, Error};
use gstreamer::{
    element_error, glib, prelude::*, DebugCategory, DebugGraphDetails, DebugLevel, DebugMessage,
    LoggedObject,
};
use image::{DynamicImage, RgbaImage};
use log::{debug, info, Level};

use crate::pix::canvas::Canvas;

#[derive(Debug, Display, Error)]
#[display("Received error from {src}: {error} (debug: {debug:?})")]
pub struct ErrorMessage {
    src: glib::GString,
    error: glib::Error,
    debug: Option<glib::GString>,
}

/// Collects video from a GStreamer pipeline and forwards it to the Pixelflut system.
pub struct GstSink {
    pipeline: gstreamer::Pipeline,
}

impl GstSink {
    /// Create a new GStreamer sink for pixelflut. This sets up all the Gstreamer internals and returns corresponding errors.
    pub fn new(
        width: u16,
        height: u16,
        pipeline_description: &str,
        canvas: Canvas,
    ) -> Result<Self> {
        let pipeline = create_pipeline(width, height, pipeline_description, canvas)?;
        Ok(Self { pipeline })
    }

    pub fn work(&mut self) -> Result<()> {
        info!("GStreamer pipeline starting...");
        self.pipeline.set_state(gstreamer::State::Playing)?;

        let bus = self
            .pipeline
            .bus()
            .ok_or(anyhow!("Pipeline without bus. Shouldn't happen!"))?;

        for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
            use gstreamer::MessageView;

            match msg.view() {
                MessageView::Eos(..) => break,
                MessageView::Error(err) => {
                    self.pipeline.set_state(gstreamer::State::Null)?;
                    return Err(ErrorMessage {
                        src: msg
                            .src()
                            .map(|s| s.path_string())
                            .unwrap_or_else(|| glib::GString::from("UNKNOWN")),
                        error: err.error(),
                        debug: err.debug(),
                    }
                    .into());
                }
                _ => (),
            }
        }

        self.pipeline.set_state(gstreamer::State::Null)?;

        Ok(())
    }
}

/// Partially based on https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/-/blob/main/examples/src/bin/appsink.rs
fn create_pipeline(
    width: u16,
    height: u16,
    pipeline_description: &str,
    mut canvas: Canvas,
) -> Result<gstreamer::Pipeline> {
    gstreamer::init()?;
    gstreamer::log::remove_default_log_function();
    gstreamer::log::add_log_function(gstreamer_log);
    // Ignore memdump messages as they create unnecessary load outside of GStreamer element debugging.
    gstreamer::log::set_default_threshold(gstreamer::DebugLevel::Trace);
    gstreamer::log::set_active(true);

    let pipeline = gstreamer::Pipeline::default();
    let bin = gstreamer::parse::bin_from_description(pipeline_description, false)?;
    debug!(
        "Built user-defined pipeline: {}",
        bin.debug_to_dot_data(DebugGraphDetails::ALL)
    );
    // Convert from any kind of pixel format.
    let videoconvert = gstreamer::ElementFactory::make("videoconvert").build()?;
    // Rescale any size of video to the size used for the pixelflut output.
    let videoscale = gstreamer::ElementFactory::make("videoscale")
        .property("n-threads", 32u32)
        .build()?;

    // Accept raw ARGB video.
    let appsink = gstreamer_app::AppSink::builder()
        .caps(
            &gstreamer_video::VideoCapsBuilder::new()
                .format(gstreamer_video::VideoFormat::Rgba)
                .width(width.into())
                .height(height.into())
                .build(),
        )
        .build();

    pipeline.add_many([
        bin.upcast_ref(),
        &videoconvert,
        &videoscale,
        appsink.upcast_ref(),
    ])?;
    videoscale.link(&videoconvert)?;
    videoconvert.link(&appsink)?;

    // User defines one element with the name pixelflut_out -- this is what we connect to
    let output_element = bin
        .iterate_elements()
        .find(|el| el.name() == "pixelflut_out");
    if let Some(output_element) = output_element {
        output_element.link(&videoscale)?;
        debug!("Linked to pixelflut_out sink element {:?}", output_element);
    } else {
        return Err(anyhow!("No element named 'pixelflut_out' found. Please set the property 'name=pixelflut_out' on your last source element."));
    }

    appsink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                // Pull the sample in question out of the appsink's buffer.
                let sample = appsink
                    .pull_sample()
                    .map_err(|_| gstreamer::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or_else(|| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to get buffer from appsink")
                    );
                    gstreamer::FlowError::Error
                })?;

                // At this point, buffer is only a reference to an existing memory region somewhere.
                // When we want to access its content, we have to map it while requesting the required
                // mode of access (read, read/write).
                // This type of abstraction is necessary, because the buffer in question might not be
                // on the machine's main memory itself, but rather in the GPU's memory.
                // So mapping the buffer makes the underlying memory region accessible to us.
                // See: https://gstreamer.freedesktop.org/documentation/plugin-development/advanced/allocation.html
                let map = buffer.map_readable().map_err(|_| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to map buffer readable")
                    );
                    gstreamer::FlowError::Error
                })?;

                // Unfortunately, we have to copy the buffer contents here since the GStreamer buffer won’t be around for long enough.
                let maybe_image = RgbaImage::from_raw(width as u32, height as u32, map.to_vec());
                if let Some(image) = maybe_image {
                    debug!("Received raw image, sending to painters...",);
                    canvas.update_image(&mut DynamicImage::from(image));
                }

                Ok(gstreamer::FlowSuccess::Ok)
            })
            .build(),
    );

    pipeline.set_state(gstreamer::State::Ready)?;
    info!(
        "GStreamer pipeline successfully created with {} elements",
        pipeline.iterate_elements().into_iter().count()
    );

    Ok(pipeline)
}

fn gstreamer_debug_level_to_log_level(level: DebugLevel) -> Option<Level> {
    match level {
        DebugLevel::Error => Some(Level::Error),
        DebugLevel::Warning => Some(Level::Warn),
        DebugLevel::Info => Some(Level::Info),
        DebugLevel::Fixme | DebugLevel::Debug => Some(Level::Debug),
        DebugLevel::Memdump | DebugLevel::Log | DebugLevel::Trace => Some(Level::Trace),
        _ => None,
    }
}

static GSTREAMER_PREFIX: LazyLock<String> = LazyLock::new(|| "gstreamer::".to_owned());

fn gstreamer_log(
    category: DebugCategory,
    level: DebugLevel,
    _file: &glib::GStr,
    _function: &glib::GStr,
    _line: u32,
    _object: Option<&LoggedObject>,
    message: &DebugMessage,
) {
    if let Some(log_level) = gstreamer_debug_level_to_log_level(level) {
        let target = GSTREAMER_PREFIX.clone() + category.name();
        log::log!(target: &target, log_level, "({}) {}", level.name().trim(), message.get().unwrap_or_default());
    }
}

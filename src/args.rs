use anyhow::{anyhow, Result};
use clap::Parser;
use image::imageops::FilterType;

use crate::image_manager::ImagePreprocessing;

#[derive(Parser)]
#[command(author, version, about, disable_help_flag = true)]
pub struct Arguments {
    // manually redefine help, but without short option, because `-h`
    // is already used by the height option.
    /// Show this help
    #[clap(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    /// The host to pwn "host:port"
    host: String,

    /// The source address to bind to
    address: Option<String>,

    #[command(flatten)]
    input: InputArguments,

    /// Draw width [default: screen width]
    #[arg(short, long, value_name = "PIXELS")]
    width: Option<u16>,
    /// Draw height [default: screen height]
    #[arg(short, long, value_name = "PIXELS")]
    height: Option<u16>,

    /// Draw X offset
    #[arg(short, value_name = "PIXELS", default_value_t = 0)]
    x: u16,
    /// Draw Y offset
    #[arg(short, value_name = "PIXELS", default_value_t = 0)]
    y: u16,

    /// Number of concurrent threads [default: number of CPUs]
    #[arg(short, long, aliases = ["thread", "threads"])]
    count: Option<usize>,

    /// Frames per second with multiple images
    #[arg(short = 'r', long, value_name = "RATE", default_value_t = 1)]
    fps: u32,

    /// Image scaling algorithm to use (dev: )
    #[arg(short, long, value_name="SCALING", default_value="gaussian", value_parser=parse_filter_type)]
    scaling: FilterType,

    /// Image preprocessing to apply. Can be one of: [none, diff, cutoff]
    #[arg(long, value_name="PROC", default_value="none", value_parser=parse_image_processing)]
    preprocessing: ImagePreprocessing,

    /// Flush socket after each pixel [default: true]
    #[arg(short, long, action = clap::ArgAction::Set, value_name = "ENABLED", default_value_t = true)]
    flush: bool,
}

#[derive(Parser)]
#[group(required = true)]
pub struct InputArguments {
    /// Image path(s)
    #[arg(
        short,
        long,
        value_name = "PATH",
        alias = "images",
        num_args(1..)
    )]
    image: Vec<String>,

    /// Gstreamer input pipeline to use.
    /// This is an alternative to input images.
    /// The pipeline format is identical to gst-launch, see
    /// https://gstreamer.freedesktop.org/documentation/tools/gst-launch.html#pipeline-description for a description.
    /// A raw video source pad (unconnected) must be available that will be output to pixelflut.
    /// This element must be named `pixelflut_out`.
    /// Any format will be scaled and converted to the draw size.
    #[cfg(feature = "gst")]
    #[arg(long, value_name = "PIPELINE")]
    pipeline: Option<String>,
}

fn parse_filter_type(arg: &str) -> Result<FilterType> {
    match arg {
        "gaussian" => Ok(FilterType::Gaussian),
        "triangle" => Ok(FilterType::Triangle),
        "catmull-rom" => Ok(FilterType::CatmullRom),
        "lanczos" => Ok(FilterType::Lanczos3),
        "nearest" => Ok(FilterType::Nearest),
        _ => Err(anyhow!("invalid image filter '{}'", arg)),
    }
}

fn parse_image_processing(arg: &str) -> Result<ImagePreprocessing> {
    match arg {
        "none" => Ok(ImagePreprocessing::None),
        "diff" => Ok(ImagePreprocessing::Diff),
        "cutoff" => Ok(ImagePreprocessing::Cutoff),
        _ => Err(anyhow!("invalid image preprocessor '{}'", arg)),
    }
}

/// CLI argument handler.
pub struct ArgHandler {
    data: Arguments,
}

impl ArgHandler {
    pub fn parse() -> ArgHandler {
        ArgHandler {
            data: Arguments::parse(),
        }
    }

    /// Get the host property.
    pub fn host(&self) -> &str {
        self.data.host.as_str()
    }

    /// Get the address property.
    pub fn address(&self) -> &Option<String> {
        &self.data.address
    }

    /// Get the scaling property.
    pub fn scaling(&self) -> FilterType {
        self.data.scaling
    }

    /// Get the image preprocessing property.
    pub fn image_preprocessing(&self) -> ImagePreprocessing {
        self.data.preprocessing
    }

    /// Get the thread count.
    pub fn count(&self) -> usize {
        self.data.count.unwrap_or_else(num_cpus::get)
    }

    /// Get the image paths.
    pub fn image_paths(&self) -> Vec<&str> {
        self.data.input.image.iter().map(|x| x.as_str()).collect()
    }

    #[cfg(feature = "gst")]
    pub fn pipeline(&self) -> Option<String> {
        self.data.input.pipeline.clone()
    }

    /// Get the image size.
    /// Use the given default value if not set.
    pub fn size(&self, def: Option<(u16, u16)>) -> (u16, u16) {
        (
            self.data
                .width
                .unwrap_or(def.expect("No screen width set or known").0),
            self.data
                .height
                .unwrap_or(def.expect("No screen height set or known").1),
        )
    }

    /// Get the image offset.
    pub fn offset(&self) -> (u16, u16) {
        (self.data.x, self.data.y)
    }

    /// Get the FPS.
    pub fn fps(&self) -> u32 {
        self.data.fps
    }

    /// Whether to flush after each pixel.
    pub fn flush(&self) -> bool {
        self.data.flush
    }
}

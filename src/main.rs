mod args;
mod color;
#[cfg(feature = "gst")]
mod gst;
mod image_manager;
mod painter;
mod pix;
mod rect;

use std::io::Error;

use args::ArgHandler;
use image_manager::ImageManager;
use pix::canvas::Canvas;
use pix::client::Client;

/// Main application entrypoint.
fn main() {
    // Parse CLI arguments
    let arg_handler = ArgHandler::parse();

    // Start
    start(&arg_handler);
}

/// Start pixelfluting.
fn start(arg_handler: &ArgHandler) {
    // Start
    println!("Starting... (use CTRL+C to stop)");

    // Gather facts about the host
    let screen_size = gather_host_facts(arg_handler).ok();

    // Determine the size to use
    let size = arg_handler.size(screen_size);

    // Load the image manager
    let mut image_manager = ImageManager::load(
        &arg_handler.image_paths(),
        size,
        arg_handler.scaling(),
        arg_handler.image_preprocessing(),
    );

    // Create a new pixelflut canvas
    let mut canvas = Canvas::new(
        arg_handler.host(),
        &arg_handler.address(),
        arg_handler.count(),
        size,
        arg_handler.offset(),
        arg_handler.binary(),
        arg_handler.flush(),
        image_manager.image_count() == 1,
    );

    #[cfg(feature = "gst")]
    if let Some(pipeline) = arg_handler.pipeline() {
        let sink = gst::GstSink::new(size.0, size.1, &pipeline, canvas);
        match sink {
            Err(why) => eprintln!("error setting up GStreamer: {}", why),
            Ok(mut sink) => {
                let result = sink.work();
                if let Err(why) = result {
                    eprintln!("error running GStreamer: {why}");
                }
            }
        }
    } else {
        // Start the work in the image manager, to walk through the frames
        image_manager.work(&mut canvas, arg_handler.fps());
    }

    #[cfg(not(feature = "gst"))]
    {
        image_manager.work(&mut canvas, arg_handler.fps());
    }
}

/// Gather important facts about the host.
fn gather_host_facts(arg_handler: &ArgHandler) -> Result<(u16, u16), Error> {
    // Set up a client, and get the screen size
    let size = Client::connect(
        arg_handler.host().to_string(),
        arg_handler.address().clone(),
        false,
        false,
    )?
    .read_screen_size()?;

    // Print status
    println!("Gathered screen size: {}x{}", size.0, size.1);

    Ok(size)
}

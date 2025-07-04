use std::net::Ipv6Addr;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use image::DynamicImage;
use log::{debug, error, info};

use crate::painter::handle::Handle;
use crate::painter::painter::Painter;
use crate::pix::client::{Pingv6Client, TextTcpClient};
use crate::rect::Rect;

use super::client::PixelClient;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Default)]
pub enum ClientType {
    #[default]
    TextTcp,
    // BinaryTcp,
    PingV6,
    // Pingxelflut,
    // TextUdp,
}

/// A pixelflut instance
pub struct Canvas {
    host: String,
    address: Option<String>,
    painter_count: usize,
    painter_handles: Vec<Handle>,
    size: (u16, u16),
    offset: (u16, u16),
    client_type: ClientType,
    should_buffer: bool,
    use_offset_command: bool,
    slowpaint: bool,
}

impl Canvas {
    /// Create a new pixelflut canvas.
    pub fn new(
        client_type: ClientType,
        host: &str,
        address: &Option<String>,
        painter_count: usize,
        size: (u16, u16),
        offset: (u16, u16),
        use_offset_command: bool,
        flush: bool,
        should_buffer: bool,
        slowpaint: bool,
    ) -> Canvas {
        // Initialize the object
        let mut canvas = Canvas {
            client_type,
            host: host.to_string(),
            address: address.clone(),
            painter_count,
            painter_handles: Vec::with_capacity(painter_count),
            size,
            offset,
            use_offset_command,
            should_buffer,
            slowpaint,
        };

        // Show a status message
        debug!("Starting painter threads...");

        // Spawn some painters
        canvas.spawn_painters(flush);

        // Return the canvas
        canvas
    }

    /// Spawn the painters for this canvas
    fn spawn_painters(&mut self, flush: bool) {
        // Spawn some painters
        for i in 0..self.painter_count {
            // Determine the slice width
            let width = self.size.0 / (self.painter_count as u16);

            // Define the area to paint per thread
            let painter_area = Rect::from((i as u16) * width, 0, width, self.size.1);

            // Spawn the painter
            self.spawn_painter(painter_area, flush);
        }
    }

    /// Spawn a single painter in a thread.
    fn spawn_painter(&mut self, area: Rect, flush: bool) {
        // Get the host that will be used
        let host = self.host.to_string();
        let address = self.address.clone();

        // Redefine values to make them usable in the thread
        let offset = (self.offset.0, self.offset.1);
        let should_buffer = self.should_buffer;

        let target_network = if self.client_type == ClientType::PingV6 {
            Ipv6Addr::from_str(&self.host).ok()
        } else {
            None
        };
        let client_type = self.client_type;
        let slowpaint = self.slowpaint;
        let use_offset_command = self.use_offset_command;

        // Create a channel to push new images
        let (tx, rx): (Sender<DynamicImage>, Receiver<DynamicImage>) = mpsc::channel();

        // Create the painter thread
        let thread = thread::spawn(move || {
            loop {
                match client_type {
                    ClientType::PingV6 => {
                        let client = Pingv6Client::new(target_network.unwrap());
                        Self::run_painter(client, area, offset, &rx, slowpaint);
                    }
                    ClientType::TextTcp => {
                        // Connect
                        match TextTcpClient::connect(
                            host.clone(),
                            address.clone(),
                            flush,
                            should_buffer,
                            if use_offset_command {
                                Some(offset)
                            } else {
                                None
                            },
                        ) {
                            Ok(client) => {
                                // only apply offset in painter if the OFFSET command is not used.
                                Self::run_painter(
                                    client,
                                    area,
                                    if use_offset_command { (0, 0) } else { offset },
                                    &rx,
                                    slowpaint,
                                );
                            }
                            Err(e) => {
                                error!("Painter failed to connect: {}", e);
                            }
                        };
                    }
                }

                // Sleep for half a second before restarting the painter
                sleep(Duration::from_millis(500));
                info!("Restarting failed painter...");
            }
        });

        // Create a new painter handle, push it to the list
        self.painter_handles.push(Handle::new(thread, area, tx));
    }

    fn run_painter(
        client: impl PixelClient,
        area: Rect,
        offset: (u16, u16),
        rx: &Receiver<DynamicImage>,
        slowpaint: bool,
    ) {
        let mut painter = Painter::new(Some(client), area, offset, None, slowpaint);

        loop {
            if let Err(e) = painter.work(rx) {
                error!("Painter error: {}", e);
                break;
            }
        }
    }

    // Update the image that is being rendered for all painters.
    pub fn update_image(&mut self, image: &mut DynamicImage) {
        // Update the image for each specific painter handle
        for handle in &self.painter_handles {
            handle.update_image(image);
        }
    }
}

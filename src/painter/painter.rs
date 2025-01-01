use anyhow::Result;
use std::fmt::Write;
use std::sync::mpsc::Receiver;

use image::{DynamicImage, Pixel};

use crate::color::Color;
use crate::pix::client::Client;
use crate::rect::Rect;

/// A painter that paints on a pixelflut panel.
pub struct Painter {
    client: Option<Client>,
    area: Rect,
    offset: (u16, u16),
    image: Option<DynamicImage>,
    buffer: String,
    should_buffer: bool,
}

impl Painter {
    /// Create a new painter.
    pub fn new(
        client: Option<Client>,
        area: Rect,
        offset: (u16, u16),
        image: Option<DynamicImage>,
        should_buffer: bool,
    ) -> Painter {
        Painter {
            client,
            area,
            offset,
            image,
            buffer: String::new(),
            should_buffer,
        }
    }

    /// Perform work.
    /// Paint the whole defined area.
    pub fn work(&mut self, img_receiver: &Receiver<DynamicImage>) -> Result<()> {
        // Wait for an image, if no image has been set yet
        if self.image.is_none() {
            // Show a warning

            // Wait for the first image to come in.
            self.set_image(img_receiver.recv()?);

            // We may now continue
            println!("Painter thread received an image, painting...");
        }

        if let Ok(image) = img_receiver.try_recv() {
            self.set_image(image);
        }

        // Get an RGB image
        let image = self.image.as_mut().unwrap().to_rgba8();

        if !self.should_buffer || self.buffer.len() == 0 {
            // Loop through all the pixels, and set their color
            for x in 0..self.area.w {
                for y in 0..self.area.h {
                    // Update the image to paint
                    if let Ok(image) = img_receiver.try_recv() {
                        self.set_image(image);
                    }

                    // Get the pixel at this location
                    let pixel = image.get_pixel(x as u32, y as u32);

                    // Get the channels
                    let channels = pixel.channels();

                    if channels[3] == 0 {
                        continue;
                    }

                    // Define the color
                    let color = Color::from(channels[0], channels[1], channels[2], channels[3]);

                    // Set the pixel
                    if self.should_buffer {
                        writeln!(
                            &mut self.buffer,
                            "PX {} {} {}",
                            x + self.area.x + self.offset.0,
                            y + self.area.y + self.offset.1,
                            color.as_hex()
                        )
                        .unwrap();
                    } else {
                        if let Some(client) = &mut self.client {
                            client.write_pixel(
                                x + self.area.x + self.offset.0,
                                y + self.area.y + self.offset.1,
                                color,
                            )?;
                        }
                    }
                }
            }
        }

        if self.should_buffer {
            if let Some(client) = &mut self.client {
                client.write_command(self.buffer.as_bytes(), true)?;
            }
        }

        // Everything seems to be ok
        Ok(())
    }

    /// Update the image that should be painted
    pub fn set_image(&mut self, image: DynamicImage) {
        self.image = Some(image);
        self.buffer.clear();
    }

    /// Update the client.
    pub fn set_client(&mut self, client: Option<Client>) {
        self.client = client;
    }
}

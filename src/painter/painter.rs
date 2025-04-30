use anyhow::anyhow;
use anyhow::Result;
use itertools::Itertools;
use log::{error, info};
use rand::seq::SliceRandom;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use image::{DynamicImage, Pixel};

use crate::color::Color;
use crate::pix::client::PixelClient;
use crate::rect::Rect;

pub const SLOWPAINT_DELAY: Duration = Duration::from_micros(1);

/// A painter that paints on a pixelflut panel.
pub struct Painter<T: PixelClient> {
    client: Option<T>,
    area: Rect,
    offset: (u16, u16),
    image: Option<DynamicImage>,
    slowpaint: bool,
}

impl<T: PixelClient> Painter<T> {
    /// Create a new painter.
    pub fn new(
        client: Option<T>,
        area: Rect,
        offset: (u16, u16),
        image: Option<DynamicImage>,
        slowpaint: bool,
    ) -> Self {
        Self {
            client,
            area,
            offset,
            image,
            slowpaint,
        }
    }

    /// Perform work.
    /// Paint the whole defined area.
    pub fn work(&mut self, img_receiver: &Receiver<DynamicImage>) -> Result<()> {
        // Wait for an image, if no image has been set yet
        if self.image.is_none() {
            // Show a warning

            // Wait for the first image to come in.
            match img_receiver.recv() {
                Ok(img) => self.set_image(img),
                Err(why) => error!("receiving first image failed: {why}"),
            }

            // We may now continue
            info!("Painter thread received an image, painting...");
        }

        if let Ok(image) = img_receiver.try_recv() {
            self.set_image(image);
        }

        // Get an RGB image
        let mut image = self.image.as_mut().ok_or(anyhow!("no image"))?.to_rgba8();
        let mut updated_image = false;

        if let Some(client) = &mut self.client {
            if self.slowpaint {
                let mut rng = rand::rng();
                let mut list = (0..self.area.w)
                    .cartesian_product(0..self.area.h)
                    .collect_vec();
                list.shuffle(&mut rng);
                for (x, y) in list {
                    if let Ok(new_image) = img_receiver.try_recv() {
                        image = new_image.into();
                        updated_image = true;
                    }
                    let channels = image.get_pixel(x as u32, y as u32).channels();
                    let color = Color::from(channels[0], channels[1], channels[2], channels[3]);
                    client.send_pixel(
                        x + self.area.x + self.offset.0,
                        y + self.area.y + self.offset.1,
                        color,
                    )?;
                    client.flush_pixels()?;
                    std::thread::sleep(SLOWPAINT_DELAY);
                }
            } else {
                // Loop through all the pixels, and set their color
                for x in 0..self.area.w {
                    for y in 0..self.area.h {
                        // Update the image to paint
                        if let Ok(new_image) = img_receiver.try_recv() {
                            image = new_image.into();
                            updated_image = true;
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
                        client.send_pixel(
                            x + self.area.x + self.offset.0,
                            y + self.area.y + self.offset.1,
                            color,
                        )?;
                    }
                }
            }
        }
        // make sure image is written back
        if updated_image {
            self.set_image(image.into());
        }

        if let Some(client) = &mut self.client {
            client.flush_pixels()?;
        }

        // Everything seems to be ok
        Ok(())
    }

    /// Update the image that should be painted
    pub fn set_image(&mut self, image: DynamicImage) {
        self.image = Some(image);

        if let Some(client) = &mut self.client {
            client.clear_buffers();
        }
    }
}

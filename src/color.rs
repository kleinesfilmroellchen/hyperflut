use std::cell::Cell;

use crate::lut::HEX_TO_STR_8;

/// Color struct.
///
/// Represents a color with RGB values from 0 to 255.
#[derive(Copy, Clone)]
pub struct Color {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) a: u8,
}
thread_local! {
    static STRING_BUFFER: Cell<String> = Cell::new("000000FF".into());
}

impl Color {
    /// Constructor.
    ///
    /// The color channels must be between 0 and 255.
    pub const fn from(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }

    pub fn write_hex(self, string: &mut String) {
        STRING_BUFFER.with(|buffer| {
            let unsafe_buffer = buffer.as_ptr();
            // SAFETY: We’re the only thread that can access `buffer`, as it is thread-local.
            let buffer = unsafe { &mut *unsafe_buffer };
            // SAFETY: All input data is ASCII and unable to yield invalid UTF-8.
            unsafe {
                buffer.as_bytes_mut()[0..2].copy_from_slice(HEX_TO_STR_8[self.r as usize]);
                buffer.as_bytes_mut()[2..4].copy_from_slice(HEX_TO_STR_8[self.g as usize]);
                buffer.as_bytes_mut()[4..6].copy_from_slice(HEX_TO_STR_8[self.b as usize]);
            }
            if self.a != 0xff {
                unsafe {
                    buffer.as_bytes_mut()[6..8].copy_from_slice(HEX_TO_STR_8[self.a as usize]);
                }
            }
            string.push_str(&buffer);
        });
    }
}

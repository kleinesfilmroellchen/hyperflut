use std::fmt::Write as _;
use std::io::prelude::*;
use std::net::{Ipv6Addr, SocketAddr, TcpStream, ToSocketAddrs};

use anyhow::{anyhow, Result};
use bufstream_fresh::BufStream;
use net2::TcpBuilder;
use regex::Regex;

use crate::color::Color;
use crate::painter::icmp::{EchoDirection, Icmp};

// The default buffer size for reading the client stream.
// - Big enough so we don't have to expand
// - Small enough to not take up to much memory
const CMD_READ_BUFFER_SIZE: usize = 32;

// The response format of the screen size from a pixelflut server.
const PIX_SERVER_SIZE_REGEX: &str = r"^(?i)\s*SIZE\s+([[:digit:]]+)\s+([[:digit:]]+)\s*$";

/// A generic pixel sending client.
/// The client handles outputting pixels via one of the multiple pixelflut protocol (variants).
pub trait PixelClient {
    /// Send a pixel with a given color at a certain position.
    fn send_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<()>;
    /// Flush the pixels. For example, buffering transports may only actually send something once this method is called.
    /// The recommendation is to call this function once per block that a painter is responsible for.
    fn flush_pixels(&mut self) -> Result<()> {
        Ok(())
    }
    /// Clear all internal buffers of the client in anticipation of new input.
    fn clear_buffers(&mut self) {}
}

/// Classical TCP text-based pixelflut client.
///
/// This client uses a stream to talk to a pixelflut panel.
/// It allows to write pixels to the panel, and read some status.
///
/// The client provides an interface for other logic to easily talk
/// to the pixelflut panel.
pub struct TextTcpClient {
    stream: BufStream<TcpStream>,

    /// Whether to flush the stream after each pixel.
    flush: bool,

    /// Buffering controls
    buffer: String,
    should_buffer: bool,
    is_buffer_ready: bool,
}

impl TextTcpClient {
    /// Create a new client instance.
    pub fn new(stream: TcpStream, flush: bool, should_buffer: bool) -> Self {
        Self {
            stream: BufStream::with_capacities(128, 8 * 1024, stream),
            flush,
            buffer: String::new(),
            should_buffer: should_buffer,
            is_buffer_ready: false,
        }
    }

    /// Create a new client instane from the given host, and connect to it.
    pub fn connect(
        host: String,
        addr: Option<impl ToSocketAddrs>,
        flush: bool,
        should_buffer: bool,
    ) -> Result<Self> {
        // Create a new stream, and instantiate the client
        Ok(Self::new(create_stream(host, addr)?, flush, should_buffer))
    }

    /// Write a pixel to the given stream.
    pub fn write_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<()> {
        self.write_command(
            format!("PX {} {} {}", x, y, color.as_hex()).as_bytes(),
            true,
        )
    }

    /// Read the size of the screen.
    pub fn read_screen_size(&mut self) -> Result<(u16, u16)> {
        // Read the screen size
        let data = self.write_read_command(b"SIZE")?;

        // Build a regex to parse the screen size
        let re = Regex::new(PIX_SERVER_SIZE_REGEX).unwrap();

        // Find captures in the data, return the result
        match re.captures(&data) {
            Some(matches) => Ok((
                matches[1]
                    .parse::<u16>()
                    .expect("Failed to parse screen width, received malformed data"),
                matches[2]
                    .parse::<u16>()
                    .expect("Failed to parse screen height, received malformed data"),
            )),
            None => Err(anyhow!(
                "Failed to parse screen size, received malformed data",
            )),
        }
    }

    /// Write the given command to the given stream.
    pub fn write_command(&mut self, cmd: &[u8], newline: bool) -> Result<()> {
        // Write the pixels and a new line
        self.stream.write_all(cmd)?;
        if newline {
            self.stream.write_all(b"\n")?;
        }

        // Flush, make sure to clear the send buffer
        // TODO: only flush each 100 pixels?
        // TODO: make buffer size configurable?
        if self.flush {
            self.stream.flush()?;
        }

        // Everything seems to be ok
        Ok(())
    }

    /// Write the given command to the given stream, and read the output.
    fn write_read_command(&mut self, cmd: &[u8]) -> Result<String> {
        // Write the command
        self.write_command(cmd, true)?;

        // Flush the pipe, ensure the command is actually sent
        self.stream.flush()?;

        // Read the output
        // TODO: this operation may get stuck (?) if nothing is received from the server
        let mut buffer = String::with_capacity(CMD_READ_BUFFER_SIZE);
        self.stream.read_line(&mut buffer)?;

        // Return the read string
        Ok(buffer)
    }
}

impl Drop for TextTcpClient {
    /// Nicely drop the connection when the client is disconnected.
    fn drop(&mut self) {
        let _ = self.write_command(b"\nQUIT", true);
    }
}

impl PixelClient for TextTcpClient {
    fn send_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<()> {
        if self.should_buffer {
            if !self.is_buffer_ready {
                writeln!(&mut self.buffer, "PX {} {} {}", x, y, color.as_hex())?;
            }
        } else {
            self.write_pixel(x, y, color)?;
        }
        Ok(())
    }

    fn flush_pixels(&mut self) -> Result<()> {
        if self.should_buffer {
            self.is_buffer_ready = true;
            // reimplement write_command() for borrow checker reasons
            self.stream.write_all(self.buffer.as_bytes())?;
            self.stream.write_all(b"\n")?;
            self.stream.flush()?;
        }
        Ok(())
    }

    fn clear_buffers(&mut self) {
        self.is_buffer_ready = false;
        self.buffer.clear();
    }
}

/// Create a stream to talk to the pixelflut server.
///
/// The stream is returned as result.
fn create_stream(host: String, addr: Option<impl ToSocketAddrs>) -> Result<TcpStream> {
    let builder = TcpBuilder::new_v4()?;
    if let Some(addr) = addr {
        builder.bind(addr)?;
    }
    let stream = builder.connect(host)?;
    Ok(stream)
}

pub struct Pingv6Client {
    target_network: [u16; 4],
}

impl Pingv6Client {
    /// 'hf' (hyperflut)
    const ID: u16 = 0x6866;

    pub fn new(target_network: Ipv6Addr) -> Self {
        Self {
            target_network: target_network.segments()[0..4].try_into().unwrap(),
        }
    }
}

impl PixelClient for Pingv6Client {
    fn send_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<()> {
        let target_address = [
            self.target_network[0],
            self.target_network[1],
            self.target_network[2],
            self.target_network[3],
            x,
            y,
            ((color.r as u16) << 8) | color.g as u16,
            (color.b as u16) << 8,
        ];
        // log::debug!("{}", Ipv6Addr::from(target_address));
        let mut packet = Icmp::new(
            SocketAddr::new(target_address.into(), 0),
            Self::ID,
            EchoDirection::Request,
        );
        let _ = packet.send()?;
        Ok(())
    }
}

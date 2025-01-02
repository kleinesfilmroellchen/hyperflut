# hyperflut

A fast and feature-rich [Pixelflut](https://github.com/defnull/pixelflut) client. This client is focused on streaming rectangular raster images and video (optionally with alpha) onto pixelflut servers as fast as possible. This is considered the “default” Pixelflut application and what many want to do with their local Pixelflut server. It does not aim to cover more specialized use cases, especially ones that can dynamically write to various different parts of the screen.

hyperflut is a hard fork of [pixelpwnr](https://timvisee.com/projects/pixelpwnr/), last synced at 38c3 (end of 2024). Many command line options are currently compatible with pixelpwnr’s syntax, but this is not guaranteed to hold in the future.

## Features

- Animated images (GIF and other multi-frame formats)
- GStreamer pipelines (user-specified with gst-launch syntax) to stream a vast variety of video sources onto pixelflut canvases
- Control over render sizes and offset
- Automatic image sizing and formatting
- Faster than most other clients :-)
- Portable; the image and animation mode supports any std environment

## Usage

Pixelflut a simple image:

```shell
# Flut a simple image.
# - To host 127.0.0.1 on port 8080
# - With the image: image.png
# - With 4 painting threads
# - With the size of the screen (default)
hyperflut 127.0.0.1:8080 -i image.png -c 4

# Other CLI syntax is also supported
hyperflut "127.0.0.1:8080" --image="image.png" -c=4
```

Pixelflut an animated image:

```shell
# Flut an animated image, with multiple frames.
# - To host 127.0.0.1 on port 8080
# - With the images: *.png
# - With 5 frames per second
# - With 4 painting threads
# - With a size of (400, 400)
# - With an offset of (100, 100)
hyperflut 127.0.0.1:8080 -i *.png --fps 5 -c 4 -w 400 -h 400 -x 100 -y 100
```

Use the `--help` flag for all available options.

### Useful GStreamer commands

If you have GStreamer-enabled hyperflut, you can use a pipeline just like in `gst-launch` to create very involved effects. The only requirement is that some kind of raw video (video/x-raw) is sourced from an element named `pixelflut_out`. While any framerate is accepted, lower framerates (<20) will generally yield better results due to general performance restrictions. Note that any X input sources do not work thanks to missing a XInitThreads() call; I couldn’t get it to work even with this so hyperflut doesn’t run this call.

Here are some useful examples to get you started.

```gstreamer
# Stream any video at 10fps (the last videoconvert node is a dummy element)
filesrc location=my_video.mkv ! decodebin ! videorate ! video/x-raw,framerate=10/1 ! videoconvert name=pixelflut_out
# Make black and dark elements of a video transparent, increasing drawing efficiency
filesrc location=my_video.mkv ! decodebin ! videorate ! videoconvert ! video/x-raw,framerate=10/1 ! alpha method=custom target-b=0 target-r=0 target-g=0 black-sensitivity=128 white-sensitivity=0 name=pixelflut_out
# Stream some Video4Linux2 source, such as your camera or a loopback device with input video from ffmpeg, with zebra stripes on bright areas
v4l2src device=/dev/videoXXX ! videorate ! video/x-raw,framerate=10/1 ! zebrastripe name=pixelflut_out
```

## Installation

Hyperflut is written in Rust and built with Cargo. It uses a stable toolchain and runs on at least the latest Rust version.

Hyperflut has some features that can be enabled and disabled depending on your needs:

- `gst`: Enables GStreamer support. This is not enabled by default since the native GStreamer libraries are required, and their installation is not possible/straightforward on all platforms. [See here](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs#installation) for the official installation instructions if you want to use GStreamer.

Clone and install `hyperflut` with:

```shell
# Clone the project
git clone https://github.com/kleinesfilmroellchen/hyperflut.git
cd hyperflut

# Install hyperflut to your system
cargo install -f
# With GStreamer support:
cargo install --features gst -f

# Start using hyperflut
hyperflut --help

# or run it directly from Cargo
cargo run --release -- --help

# After building once, you can also use:
./target/release/hyperflut --help
```

You can configure hyperflut’s logging by using the RUST_LOG environment variable. Its general syntax is [described here](https://docs.rs/env_logger/latest/env_logger/#example). All GStreamer logging categories are nested under the `gstreamer` logging module, so e.g. to enable debug output for GStreamer’s Video4Linux2 elements you could set `RUST_LOG=gstreamer::v4l2=DEBUG`.

## Performance & speed optimization

There are many things that affect how quickly pixels can be painted on a
pixelflut server.
Some of them are:

- Size of the image that is drawn.
- Amount of connections used to push pixels.
- Performance of the machine `hyperflut` is running on.
- Network interface performance of the client.
- Network interface performance of the server.
- Performance of the pixelflut server.

Things that improve painting performance:

- Use a wired connection. Most Pixelflut setups at CCC events nowadays block all wireless traffic anyways.
- Use a LAN connection closely linked to the pixelflut server. The lower latency the better, due to the connection being over TCP.
- Use as many threads (`-c` flag) as the server, your connection and your machine allows. Many servers at events heavily limit the connection count per IP, e.g. one of the GPN21 servers had a limit of 1 connection/IP and the 38c3 server had a limit of 2 connections/IP.
- Paint a smaller image (`-w`, `-h` flags).
- Paint in an area on the screen where the least other things are painted.
- Use multiple machines with multiple `hyperflut` instances to push pixels to the screen.

Performance improvements over other implementations that have been implemented in hyperflut:

- Separated handling of image decoding and processing versus painting.
- Arbitrarily multithreaded painting. This is only an advantage on servers that allow multiple connections.
- Pixelflut command buffering. For static images, this vastly improves performance even over pixelpwnr, easily saturating multi-gigabit links with one or two painter threads.
- Discarding of transparent pixels. In combination with video processing in GStreamer, this allows you to filter out only the relevant pixels and draw large-scale graphics with little bandwidth requirements.

## License

This project is released under the GNU GPL-3.0 license. Check out the [LICENSE](LICENSE) file for more information.

Since the GPL’d code from pixelpwnr cannot be relicensed, unfortunately I cannot offer a different license than this. However, all source files completely written by me are available under the Public Domain [Unlicense](UNLICENSE), this is noted in the file’s header comment when applicable.

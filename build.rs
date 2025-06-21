use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let output_path = PathBuf::from_iter([std::env::var("OUT_DIR").unwrap(), "lut.rs".into()]);
    let mut output_file = BufWriter::new(
        File::options()
            .write(true)
            .create(true)
            .open(output_path)
            .unwrap(),
    );

    output_file
        .write_all("pub const HEX_TO_STR_8: &[&[u8;2]] = &[\n".as_bytes())
        .unwrap();

    for byte in 0..=0xff {
        output_file
            .write_all(format!("b\"{byte:02X}\",").as_bytes())
            .unwrap();
    }
    output_file.write_all("];\npub const HEX_TO_STR_16: &[&str] = &[\n".as_bytes()).unwrap();

    // Only do a few values since we haven’t yet had to deal with canvases larger than a few thousand pixels in width/height.
    for value in 0..=5000 {
        output_file
            .write_all(format!("\"{value}\",").as_bytes())
            .unwrap();
    }
    output_file.write_all("];\n".as_bytes()).unwrap();
}

//! Make the exact release image checksum manifest an executable build input.
//! Source/developer builds are supported without the optional compact artifact.
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from("dist/Image/SHA256SUMS.txt");
    println!("cargo:rerun-if-changed={}", path.display());
    let packaged = match fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("Cannot read the compact image manifest: {error}"),
    };
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
        .join("packaged_image_hashes.txt");
    fs::write(output, packaged).expect("Failed to embed compact image digests");
}

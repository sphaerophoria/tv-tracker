use std::{fs::OpenOptions, path::Path};

const CLIENT_PATH: &str = "res/client";

fn main() {
    println!("cargo:rerun-if-changed={CLIENT_PATH}");
    let client_tarball_path =
        Path::new(&std::env::var("OUT_DIR").expect("Failed to find OUT_DIR")).join("client.tar");

    let client_tarball_writer = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(client_tarball_path)
        .expect("Failed to open client tarball");

    let mut tarball = tar::Builder::new(client_tarball_writer);
    tarball
        .append_dir_all(".", CLIENT_PATH)
        .expect("Failed to create tarball");
}

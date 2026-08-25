fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let descriptor_path = std::path::PathBuf::from(out_dir).join("messenger_descriptor.bin");
    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path(descriptor_path)
        .compile_protos(&["proto/messenger.proto"], &["proto"])?;
    Ok(())
}

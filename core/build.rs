use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../proto");
    let proto_file = proto_dir.join("myb.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());

    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&[proto_file], &[proto_dir])?;

    Ok(())
}

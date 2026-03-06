fn main() -> anyhow::Result<()> {
    let proto_file = "../../proto/kernel.proto";
    let siren_file = "../../proto/siren.proto";

    // Rerun build if proto changes
    println!("cargo:rerun-if-changed={}", proto_file);
    println!("cargo:rerun-if-changed={}", siren_file);

    // Compile with Serde support only for our core messages
    tonic_build::configure()
        .type_attribute("ank.v1", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&[proto_file, siren_file], &["../../proto"])?;

    Ok(())
}

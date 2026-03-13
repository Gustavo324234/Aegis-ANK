fn main() -> anyhow::Result<()> {
    let proto_file = "../../proto/kernel.proto";
    let siren_file = "../../proto/siren.proto";

    // Forced cache cleanup as requested by SRE Firewall [ANK-905]
    if let Ok(out_dir) = std::env::var("OUT_DIR") {
        let _ = std::fs::remove_dir_all(&out_dir);
        let _ = std::fs::create_dir_all(&out_dir);
    }

    // Rerun build if proto changes
    println!("cargo:rerun-if-changed={}", proto_file);
    println!("cargo:rerun-if-changed={}", siren_file);

    // Compile with explicit boxing for large variants
    let mut config = prost_build::Config::new();
    config.type_attribute("ank.v1", "#[derive(serde::Serialize, serde::Deserialize)]");

    // Use .boxed(path) for reliable variant boxing (prost-build 0.13 method)
    config.boxed(".ank.v1.Payload.status_update");
    config.boxed(".ank.v1.TaskEvent.payload.status_update");

    tonic_build::configure().compile_protos_with_config(
        config,
        &[proto_file, siren_file],
        &["../../proto"],
    )?;

    Ok(())
}

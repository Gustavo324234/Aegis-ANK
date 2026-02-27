fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Le dice a Cargo que recompile si el archivo .proto cambia
    println!("cargo:rerun-if-changed=../../proto/kernel.proto");
    
    tonic_build::compile_protos("../../proto/kernel.proto")?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/bridge.proto");
    println!("cargo:rerun-if-changed=proto/machiavelli.proto");
    let descriptor = protox::compile(["proto/bridge.proto", "proto/machiavelli.proto"], ["proto"])?;
    tonic_prost_build::configure().compile_fds(descriptor)?;
    Ok(())
}

// This is a build script for the Kore Sink client

fn main () -> Result<(), Box<dyn std::error::Error>> {
  tonic_prost_build::compile_protos("../protos/sink.proto")?;
  Ok(())
}

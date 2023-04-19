fn main () -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile(&["./protos/prediction.proto"], &["./protos"])?;
    //tonic_build::compile_protos(proto)
    Ok(())
  }
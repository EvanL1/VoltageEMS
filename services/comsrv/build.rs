use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 只在 proto fileexists时才compiling
    let proto_file = PathBuf::from("proto/protocol_plugin.proto");
    if proto_file.exists() {
        tonic_build::configure()
            .build_server(false) // 只需要client
            .build_client(true)
            .out_dir("src/plugins/grpc/proto")
            .compile(&["proto/protocol_plugin.proto"], &["proto"])?;
    }
    Ok(())
}

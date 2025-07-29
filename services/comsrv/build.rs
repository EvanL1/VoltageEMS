use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 只在 proto 文件存在时才编译
    let proto_file = PathBuf::from("proto/protocol_plugin.proto");
    if proto_file.exists() {
        tonic_build::configure()
            .build_server(false) // 只需要客户端
            .build_client(true)
            .out_dir("src/plugins/grpc/proto")
            .compile(&["proto/protocol_plugin.proto"], &["proto"])?;
    }
    Ok(())
}

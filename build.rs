fn main() {
    let mut config = prost_build::Config::new();
    config.protoc_arg("--experimental_allow_proto3_optional");
    tonic_build::configure()
        .compile_with_config(config, &["proto/published_packet.proto"], &["proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}

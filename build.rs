fn main() {
    let proto_files = ["proto/synagraph.proto"];
    let proto_includes = ["proto"];

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&proto_files, &proto_includes)
        .expect("failed to compile protobuf definitions");
}

// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Build scripts regenerate protobuf bindings so API updates stay in sync across languages.

fn main() {
    let proto_files = ["proto/synagraph.proto"];
    let proto_includes = ["proto"];

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&proto_files, &proto_includes)
        .expect("failed to compile protobuf definitions");
}

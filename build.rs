use protobuf_codegen::Codegen;

fn main() {
    Codegen::new()
        .pure()
        .out_dir("src/")
        .include("src")
        .input("src/messages.proto")
        .run()
        .unwrap();
}

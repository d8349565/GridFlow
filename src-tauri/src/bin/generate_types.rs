fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    excel_workbench::model::generate_types(&root.join("src/types.ts"))
        .expect("generate TypeScript contracts");
}

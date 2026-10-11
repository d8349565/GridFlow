fn main() {
    let rc = std::env::var_os("WORKBENCH_LLVM_RC").expect("LLVM resource compiler path");
    let status = std::process::Command::new(rc)
        .args(["/C", "65001"])
        .args(std::env::args_os().skip(1))
        .status().expect("run LLVM resource compiler");
    std::process::exit(status.code().unwrap_or(1));
}

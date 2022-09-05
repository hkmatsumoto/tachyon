#![feature(rustc_private)]

extern crate rustc_session;

use std::path::PathBuf;

fn run_mode(mode: &'static str) {
    let mut config = compiletest_rs::Config::default().tempdir();
    let mode = mode.parse().unwrap();

    let triple = rustc_session::config::host_triple();
    let sysroot = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap()
        .stdout;
    let sysroot = String::from_utf8(sysroot).unwrap();
    let sysroot = std::path::Path::new(sysroot.trim());
    let lib_path = rustc_session::filesearch::make_target_lib_path(sysroot, triple);

    config.rustc_path = PathBuf::from("target/debug/tachyon");
    config.mode = mode;
    config.src_base = PathBuf::from(format!("tests/{}", mode));

    config.target_rustcflags = Some(format!(
        "-C overflow-checks=off --crate-type=lib -L {}",
        lib_path.to_str().unwrap()
    ));
    config.clean_rmeta();

    compiletest_rs::run_tests(&config);
}

#[test]
fn compile_test() {
    run_mode("ui");
}

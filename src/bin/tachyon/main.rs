#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;

use std::{fs::OpenOptions, io::Write, path::PathBuf, process::Command};

use rustc_driver::Callbacks;

pub mod backend;
pub mod shims;

struct TachyonCallbacks;
impl Callbacks for TachyonCallbacks {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> rustc_driver::Compilation {
        queries
            .global_ctxt()
            .unwrap()
            .peek_mut()
            .enter(|tcx| shims::compile_testfile(tcx));

        rustc_driver::Compilation::Stop
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    let callbacks = &mut TachyonCallbacks;
    let mut compiler = rustc_driver::RunCompiler::new(args.as_slice(), callbacks);
    compiler.set_make_codegen_backend(Some(Box::new(|_| Box::new(backend::DummyBackend))));
    compiler.run().unwrap();
}

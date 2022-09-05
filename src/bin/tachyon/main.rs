#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;

use std::{fs::OpenOptions, io::Write, path::PathBuf, process::Command};

use rustc_driver::Callbacks;

pub mod backend;
pub mod shims;

enum RunMode {
    Compile,
    Execute,
}

impl Callbacks for RunMode {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> rustc_driver::Compilation {
        queries
            .global_ctxt()
            .unwrap()
            .peek_mut()
            .enter(|tcx| match self {
                RunMode::Compile => shims::compile(tcx),
                RunMode::Execute => shims::execute(tcx),
            });

        rustc_driver::Compilation::Stop
    }
}

fn main() {
    let mut args = std::env::args().collect::<Vec<_>>();

    let mut callbacks = if args.get(1).map_or(false, |arg| arg == "--execute") {
        args.remove(1);
        RunMode::Execute
    } else {
        RunMode::Compile
    };

    let mut compiler = rustc_driver::RunCompiler::new(args.as_slice(), &mut callbacks);
    compiler.set_make_codegen_backend(Some(Box::new(|_| Box::new(backend::DummyBackend))));
    compiler.run().unwrap();
}

extern crate rustc_hir;
extern crate rustc_middle;

use llvm_sys::core::{LLVMContextCreate, LLVMDumpModule, LLVMModuleCreateWithNameInContext};
use rustc_hir::def::DefKind;
use rustc_middle::ty::TyCtxt;

use std::ffi::CString;

use tachyon::{
    c_string,
    codegen::{codegen_fn, execute_fn, optimize_fn},
};

pub fn execute<'tcx>(tcx: TyCtxt<'tcx>) {
    let mut funcs = tcx
        .hir_crate_items(())
        .items()
        .filter(|item| tcx.def_kind(item.def_id) == DefKind::Fn);
    // FIXME: Don't assume test files have only one function defined in it.
    if let Some(func) = funcs.next() {
        let func = func.def_id.to_def_id();
        let func_name = c_string!(tcx.def_path_str(func));

        unsafe {
            let llcx = LLVMContextCreate();
            let llmod = LLVMModuleCreateWithNameInContext(c_string!("top").as_ptr(), llcx);

            codegen_fn(llcx, llmod, tcx, func);
            let ee = optimize_fn(llmod);
            LLVMDumpModule(llmod);

            execute_fn(ee, func_name);
        }
    }
}

pub fn compile<'tcx>(tcx: TyCtxt<'tcx>) {
    let mut funcs = tcx
        .hir_crate_items(())
        .items()
        .filter(|item| tcx.def_kind(item.def_id) == DefKind::Fn);
    // FIXME: Don't assume test files have only one function defined in it.
    if let Some(func) = funcs.next() {
        let func = func.def_id.to_def_id();

        unsafe {
            let llcx = LLVMContextCreate();
            let llmod = LLVMModuleCreateWithNameInContext(c_string!("top").as_ptr(), llcx);

            codegen_fn(llcx, llmod, tcx, func);
            LLVMDumpModule(llmod);
        }
    }
}

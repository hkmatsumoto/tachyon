extern crate rustc_hir;
extern crate rustc_middle;

use llvm_sys::core::{LLVMContextCreate, LLVMDumpModule, LLVMModuleCreateWithNameInContext};
use rustc_hir::def::DefKind;
use rustc_middle::ty::TyCtxt;

use std::ffi::CString;

use tachyon::codegen::codegen_fn;

pub fn compile_testfile<'tcx>(tcx: TyCtxt<'tcx>) {
    let mut funcs = tcx
        .hir_crate_items(())
        .items()
        .filter(|item| tcx.def_kind(item.def_id) == DefKind::Fn);
    // FIXME: Don't assume test files have only one function defined in it.
    if let Some(func) = funcs.next() {
        let func = func.def_id.to_def_id();

        let llcx = unsafe { LLVMContextCreate() };
        let llmod =
            unsafe { LLVMModuleCreateWithNameInContext(tachyon::c_string!("top").as_ptr(), llcx) };
        unsafe { codegen_fn(llcx, llmod, tcx, func) };
        unsafe { LLVMDumpModule(llmod) };
    }
}

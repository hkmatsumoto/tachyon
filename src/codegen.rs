use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use llvm_sys::{
    core::{
        LLVMAddFunction, LLVMCreateBuilderInContext, LLVMDumpValue, LLVMGetParam, LLVMSetValueName2,
    },
    prelude::*,
};

use std::{cell::OnceCell, ffi::CString};

use crate::{ty::fn_sig_to_llvm_fn_type, FunctionCx, TPlace};

macro_rules! c_string {
    ($string:expr) => {
        CString::new($string).unwrap()
    };
}

pub unsafe fn codegen_fn<'tcx>(
    llcx: LLVMContextRef,
    llmod: LLVMModuleRef,
    tcx: TyCtxt<'tcx>,
    fn_id: DefId,
) {
    let fn_sig = tcx.fn_sig(fn_id).no_bound_vars().unwrap();
    let llfn_sig = fn_sig_to_llvm_fn_type(llcx, fn_sig);

    let fn_name = tcx.def_path_str(fn_id);
    let fn_name = c_string!(fn_name);
    let llfn = LLVMAddFunction(llmod, fn_name.as_ptr(), llfn_sig);

    let mir = tcx.optimized_mir(fn_id);

    let mut fx = FunctionCx::new(llcx, llmod, llfn, tcx, mir);
    fx.codegen_header();
    fx.codegen_body();
}

impl<'tcx> FunctionCx<'tcx> {
    unsafe fn codegen_header(&mut self) {
        dbg!(&self.mir.var_debug_info);
        for _ in &self.mir.local_decls {
            self.locals.push(TPlace {
                llval: OnceCell::new(),
            });
        }
        for arg_local in self.mir.args_iter() {
            let param = LLVMGetParam(self.llfn, arg_local.index() as core::ffi::c_uint);
        }
    }

    fn codegen_body(&mut self) {}
}

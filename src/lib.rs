#![feature(rustc_private)]
#![feature(once_cell)]
#![feature(box_patterns)]

extern crate rustc_const_eval;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;

use std::cell::OnceCell;

use rustc_index::vec::IndexVec;
use rustc_middle::{
    mir,
    ty::{layout::TyAndLayout, TyCtxt},
};

use llvm_sys::{core::*, prelude::*};

pub mod codegen;
pub(crate) mod ty;

#[derive(Debug)]
pub(crate) struct TPlace<'tcx> {
    ty_and_layout: TyAndLayout<'tcx>,
    llval: OnceCell<LLVMValueRef>,
}

pub(crate) struct FunctionCx<'tcx> {
    pub(crate) llcx: LLVMContextRef,
    pub(crate) llmod: LLVMModuleRef,
    pub(crate) llbx: LLVMBuilderRef,
    pub(crate) llfn: LLVMValueRef,

    pub(crate) tcx: TyCtxt<'tcx>,
    pub(crate) mir: &'tcx mir::Body<'tcx>,

    pub(crate) locals: IndexVec<mir::Local, TPlace<'tcx>>,
    pub(crate) basic_blocks: IndexVec<mir::BasicBlock, LLVMBasicBlockRef>,
}

impl<'tcx> FunctionCx<'tcx> {
    pub unsafe fn new(
        llcx: LLVMContextRef,
        llmod: LLVMModuleRef,
        llfn: LLVMValueRef,
        tcx: TyCtxt<'tcx>,
        mir: &'tcx mir::Body<'tcx>,
    ) -> FunctionCx<'tcx> {
        let llbx = LLVMCreateBuilderInContext(llcx);

        FunctionCx {
            llcx,
            llmod,
            llbx,
            llfn,
            tcx,
            mir,
            locals: IndexVec::with_capacity(mir.local_decls.len()),
            basic_blocks: IndexVec::with_capacity(mir.basic_blocks.raw.len()),
        }
    }
}

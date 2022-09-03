use rustc_const_eval::interpret::ConstValue;
use rustc_hir::def_id::DefId;
use rustc_middle::{
    mir::{self, traversal, Operand, Rvalue, Statement, StatementKind, Terminator, TerminatorKind},
    ty::{ParamEnv, TyCtxt, TyKind},
};

use llvm_sys::{core::*, prelude::*};

use std::{cell::OnceCell, ffi::CString};

use crate::{
    ty::{fn_sig_to_llvm_fn_type, ty_to_llvm_type},
    FunctionCx, TPlace,
};

#[macro_export]
macro_rules! c_string {
    ($string:expr) => {
        CString::new($string).unwrap()
    };
    ($prefix:literal, $string:expr) => {
        CString::new(format!("{}{}", $prefix, $string)).unwrap()
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
        for local_decl in &self.mir.local_decls {
            self.locals.push(TPlace {
                ty_and_layout: self
                    .tcx
                    .layout_of(ParamEnv::reveal_all().and(local_decl.ty))
                    .unwrap(),
                llval: OnceCell::new(),
            });
        }
        for (idx, arg_local) in self.mir.args_iter().enumerate() {
            let param = LLVMGetParam(self.llfn, idx as core::ffi::c_uint);
            let param_name = &self.mir.var_debug_info[idx].name;
            let param_name = c_string!(param_name.to_ident_string());
            LLVMSetValueName2(param, param_name.as_ptr(), param_name.as_bytes().len());

            self.locals[arg_local].llval.set(param).unwrap();
        }

        LLVMAppendBasicBlockInContext(self.llcx, self.llfn, c_string!("entry").as_ptr());
        for (bb, _) in traversal::reverse_postorder(&self.mir) {
            let block = LLVMAppendBasicBlockInContext(
                self.llcx,
                self.llfn,
                c_string!("bb", bb.index()).as_ptr(),
            );
            self.basic_blocks.push(block);
        }

        self.codegen_header_allocas();
    }

    unsafe fn alloca(&mut self, local: mir::Local) -> Option<LLVMValueRef> {
        let place = &self.locals[local];
        if place.ty_and_layout.is_zst() {
            None
        } else {
            Some(LLVMBuildAlloca(
                self.llbx,
                ty_to_llvm_type(self.llcx, place.ty_and_layout.ty),
                c_string!("_", local.index()).as_ptr(),
            ))
        }
    }

    unsafe fn codegen_header_allocas(&mut self) {
        let entry = LLVMGetEntryBasicBlock(self.llfn);
        LLVMPositionBuilderAtEnd(self.llbx, entry);

        for local in self
            .mir
            .local_decls
            .indices()
            .skip(1 + self.mir.arg_count)
            .chain(Some(mir::RETURN_PLACE))
        {
            if let Some(alloca) = self.alloca(local) {
                self.locals[local].llval.set(alloca).unwrap();
            }
        }
        LLVMBuildBr(self.llbx, LLVMGetNextBasicBlock(entry));
    }

    unsafe fn codegen_body(&mut self) {
        for (bb, data) in traversal::reverse_postorder(&self.mir) {
            LLVMPositionBuilderAtEnd(self.llbx, self.basic_blocks[bb]);

            for stmt in &data.statements {
                self.codegen_statement(stmt);
            }

            self.codegen_terminator(data.terminator());
        }
    }

    unsafe fn codegen_statement(&mut self, stmt: &Statement<'tcx>) {
        match &stmt.kind {
            StatementKind::Assign(box (place, rvalue)) => {
                let name = c_string!("_", place.local.index());
                match rvalue {
                    Rvalue::Use(operand) => {
                        let operand = self.codegen_operand(operand);
                        if let Some(llval) = self.locals[place.local].llval.get() {
                            LLVMBuildStore(self.llbx, operand, *llval);
                        } else {
                            todo!()
                        }
                    }
                    _ => todo!(),
                }
            }
            _ => todo!(),
        }
    }

    unsafe fn codegen_operand(&mut self, operand: &Operand<'tcx>) -> LLVMValueRef {
        match operand {
            mir::Operand::Constant(constant) => match constant.literal {
                mir::ConstantKind::Val(val, ty) => match val {
                    ConstValue::Scalar(rustc_const_eval::interpret::Scalar::Int(int)) => {
                        let val = int.to_bits(int.size()).unwrap() as u64;
                        let int_type = match int.size().bits() {
                            1 => LLVMInt1TypeInContext(self.llcx),
                            8 => LLVMInt8TypeInContext(self.llcx),
                            16 => LLVMInt16TypeInContext(self.llcx),
                            32 => LLVMInt32TypeInContext(self.llcx),
                            64 => LLVMInt128TypeInContext(self.llcx),
                            128 => todo!(),
                            _ => unreachable!(),
                        };
                        LLVMConstInt(int_type, val, 0)
                    }
                    _ => todo!(),
                },
                _ => todo!(),
            },
            _ => todo!(),
        }
    }

    unsafe fn codegen_terminator(&mut self, term: &Terminator<'tcx>) {
        match &term.kind {
            TerminatorKind::Return => {
                let ret_data = &self.locals[mir::RETURN_PLACE];
                match ret_data.ty_and_layout.ty.kind() {
                    TyKind::Tuple(tuple) if tuple.len() == 0 => LLVMBuildRetVoid(self.llbx),
                    _ => {
                        let ret = LLVMBuildLoad2(
                            self.llbx,
                            ty_to_llvm_type(self.llcx, ret_data.ty_and_layout.ty),
                            *ret_data.llval.get().unwrap(),
                            c_string!("").as_ptr(),
                        );
                        LLVMBuildRet(self.llbx, ret)
                    }
                };
            }
            _ => {
                todo!()
            }
        }
    }
}

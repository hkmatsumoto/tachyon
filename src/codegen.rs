use rustc_const_eval::interpret::ConstValue;
use rustc_hir::def_id::DefId;
use rustc_middle::{
    mir::{
        self, traversal, BinOp, Operand, Rvalue, Statement, StatementKind, Terminator,
        TerminatorKind,
    },
    ty::{layout::TyAndLayout, ParamEnv, Ty, TyCtxt, TyKind},
};

use llvm_sys::{
    core::*,
    execution_engine::{LLVMCreateExecutionEngineForModule, LLVMLinkInMCJIT},
    prelude::*,
    target::LLVM_InitializeNativeTarget,
    transforms::scalar::LLVMAddScalarReplAggregatesPass,
    LLVMTypeKind,
};

use std::{cell::OnceCell, ffi::CString};

use crate::{
    ty::{fn_sig_to_llvm_fn_type, ty_to_llvm_type},
    FunctionCx,
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
        for (local, local_decl) in self.mir.local_decls.iter_enumerated() {
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

        for local in Some(mir::RETURN_PLACE)
            .into_iter()
            .chain(self.mir.local_decls.indices().skip(1 + self.mir.arg_count))
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
                let name = c_string!("");
                match rvalue {
                    Rvalue::Use(operand) => {
                        self.codegen_operand(operand)
                            .load_scalar(self.llbx)
                            .store(self.llbx, *self.locals[place.local].llval());
                    }
                    Rvalue::BinaryOp(bin_op, box (lhs, rhs)) => {
                        let lhs_ty = lhs.ty(&self.mir.local_decls, self.tcx);
                        let rhs_ty = rhs.ty(&self.mir.local_decls, self.tcx);
                        let lhs_val = *self.codegen_operand(lhs).load_scalar(self.llbx).llval();
                        let rhs_val = *self.codegen_operand(rhs).load_scalar(self.llbx).llval();

                        let tmp = match bin_op {
                            BinOp::Add if lhs_ty.is_integral() => {
                                LLVMBuildAdd(self.llbx, lhs_val, rhs_val, name.as_ptr())
                            }
                            BinOp::Sub if lhs_ty.is_integral() => {
                                LLVMBuildSub(self.llbx, lhs_val, rhs_val, name.as_ptr())
                            }
                            BinOp::Eq if lhs_ty.is_integral() => LLVMBuildICmp(
                                self.llbx,
                                llvm_sys::LLVMIntPredicate::LLVMIntEQ,
                                lhs_val,
                                rhs_val,
                                name.as_ptr(),
                            ),
                            BinOp::Lt if lhs_ty.is_integral() && lhs_ty.is_signed() => {
                                LLVMBuildICmp(
                                    self.llbx,
                                    llvm_sys::LLVMIntPredicate::LLVMIntSLT,
                                    lhs_val,
                                    rhs_val,
                                    name.as_ptr(),
                                )
                            }
                            BinOp::Lt if lhs_ty.is_integral() => LLVMBuildICmp(
                                self.llbx,
                                llvm_sys::LLVMIntPredicate::LLVMIntULT,
                                lhs_val,
                                rhs_val,
                                name.as_ptr(),
                            ),
                            BinOp::Le if lhs_ty.is_integral() && lhs_ty.is_signed() => {
                                LLVMBuildICmp(
                                    self.llbx,
                                    llvm_sys::LLVMIntPredicate::LLVMIntSLE,
                                    lhs_val,
                                    rhs_val,
                                    name.as_ptr(),
                                )
                            }
                            BinOp::Le if lhs_ty.is_integral() => LLVMBuildICmp(
                                self.llbx,
                                llvm_sys::LLVMIntPredicate::LLVMIntULE,
                                lhs_val,
                                rhs_val,
                                name.as_ptr(),
                            ),
                            _ => todo!(),
                        };

                        LLVMBuildStore(self.llbx, tmp, *self.locals[place.local].llval());
                    }
                    _ => todo!(),
                }
            }
            _ => todo!(),
        }
    }

    unsafe fn codegen_operand(&mut self, operand: &Operand<'tcx>) -> TPlace<'tcx> {
        match operand {
            mir::Operand::Copy(place) | mir::Operand::Move(place) => {
                let layout = self.locals[place.local].ty_and_layout;
                let llval = self.locals[place.local].llval.clone();

                TPlace {
                    ty_and_layout: layout,
                    llval,
                }
            }
            mir::Operand::Constant(constant) => {
                let layout = self
                    .tcx
                    .layout_of(ParamEnv::reveal_all().and(constant.ty()))
                    .unwrap();
                let llval = match constant.literal {
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
                };

                TPlace {
                    ty_and_layout: layout,
                    llval: OnceCell::from(llval),
                }
            }
        }
    }

    unsafe fn codegen_terminator(&mut self, term: &Terminator<'tcx>) {
        match &term.kind {
            TerminatorKind::Goto { target } => {
                LLVMBuildBr(self.llbx, self.basic_blocks[*target]);
            }
            TerminatorKind::SwitchInt {
                discr,
                targets,
                ..
            } => {
                let operand = self.codegen_operand(discr).load_scalar(self.llbx);
                let switch = LLVMBuildSwitch(
                    self.llbx,
                    *operand.llval(),
                    self.basic_blocks[targets.otherwise()],
                    targets.all_targets().len() as std::os::raw::c_uint,
                );

                for (value, target) in targets.iter() {
                    LLVMAddCase(
                        switch,
                        LLVMConstInt(LLVMTypeOf(*operand.llval()), value as u64, 0),
                        self.basic_blocks[target],
                    );
                }
            }
            TerminatorKind::Return => {
                let ret_data = &self.locals[mir::RETURN_PLACE];
                match ret_data.ty().kind() {
                    TyKind::Tuple(tuple) if tuple.len() == 0 => LLVMBuildRetVoid(self.llbx),
                    _ => {
                        let ret = *ret_data.clone().load_scalar(self.llbx).llval();
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

#[derive(Debug, Clone)]
pub(crate) struct TPlace<'tcx> {
    ty_and_layout: TyAndLayout<'tcx>,
    llval: OnceCell<LLVMValueRef>,
}

impl<'tcx> TPlace<'tcx> {
    pub(crate) fn ty(&self) -> Ty<'tcx> {
        self.ty_and_layout.ty
    }

    pub(crate) fn llval(&self) -> &LLVMValueRef {
        self.llval.get().unwrap()
    }

    pub(crate) unsafe fn load_scalar(self, llbx: LLVMBuilderRef) -> Self {
        let llval = self.llval();
        let llval_type = LLVMTypeOf(*llval);
        if matches!(
            LLVMGetTypeKind(llval_type),
            LLVMTypeKind::LLVMPointerTypeKind
        ) {
            let llval_element_type = LLVMGetElementType(llval_type);
            let llval = LLVMBuildLoad2(llbx, llval_element_type, *llval, c_string!("").as_ptr());

            return TPlace {
                ty_and_layout: self.ty_and_layout,
                llval: OnceCell::from(llval),
            };
        }

        self
    }

    pub(crate) unsafe fn store(self, llbx: LLVMBuilderRef, ptr: LLVMValueRef) {
        let llval = self.llval();
        LLVMBuildStore(llbx, *llval, ptr);
    }
}

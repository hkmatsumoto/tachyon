use rustc_middle::ty::{FloatTy, FnSig, IntTy, Ty, TyKind, UintTy};

use llvm_sys::{core::*, prelude::*};

pub(crate) unsafe fn ty_to_llvm_type<'tcx>(llcx: LLVMContextRef, ty: Ty<'tcx>) -> LLVMTypeRef {
    match ty.kind() {
        TyKind::Bool => todo!(),
        TyKind::Char => LLVMInt32TypeInContext(llcx),
        TyKind::Int(int) => match int {
            IntTy::Isize => todo!(),
            IntTy::I8 => LLVMInt8TypeInContext(llcx),
            IntTy::I16 => LLVMInt16TypeInContext(llcx),
            IntTy::I32 => LLVMInt32TypeInContext(llcx),
            IntTy::I64 => LLVMInt64TypeInContext(llcx),
            IntTy::I128 => LLVMInt128TypeInContext(llcx),
        },
        TyKind::Uint(uint) => match uint {
            UintTy::Usize => todo!(),
            UintTy::U8 => LLVMInt8TypeInContext(llcx),
            UintTy::U16 => LLVMInt16TypeInContext(llcx),
            UintTy::U32 => LLVMInt32TypeInContext(llcx),
            UintTy::U64 => LLVMInt64TypeInContext(llcx),
            UintTy::U128 => LLVMInt128TypeInContext(llcx),
        },
        TyKind::Float(float) => match float {
            FloatTy::F32 => LLVMFloatTypeInContext(llcx),
            FloatTy::F64 => LLVMDoubleTypeInContext(llcx),
        },
        TyKind::Tuple(tuple) if tuple.len() == 0 => LLVMVoidTypeInContext(llcx),
        _ => todo!(),
    }
}

pub(crate) unsafe fn fn_sig_to_llvm_fn_type<'tcx>(
    llcx: LLVMContextRef,
    fn_sig: FnSig<'tcx>,
) -> LLVMTypeRef {
    let mut inputs = fn_sig
        .inputs()
        .iter()
        .map(|input| ty_to_llvm_type(llcx, *input))
        .collect::<Vec<_>>();
    let output = ty_to_llvm_type(llcx, fn_sig.output());

    LLVMFunctionType(
        output,
        inputs.as_mut_ptr(),
        inputs.len() as core::ffi::c_uint,
        0,
    )
}

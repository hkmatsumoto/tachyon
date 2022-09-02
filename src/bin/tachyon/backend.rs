extern crate rustc_codegen_ssa;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;

use rustc_codegen_ssa::traits::CodegenBackend;

pub struct DummyBackend;

impl CodegenBackend for DummyBackend {
    fn codegen_crate<'tcx>(
        &self,
        _tcx: rustc_middle::ty::TyCtxt<'tcx>,
        _metadata: rustc_metadata::EncodedMetadata,
        _need_metadata_module: bool,
    ) -> Box<dyn std::any::Any> {
        unreachable!()
    }

    fn join_codegen(
        &self,
        _ongoing_codegen: Box<dyn std::any::Any>,
        _sess: &rustc_session::Session,
        _outputs: &rustc_session::config::OutputFilenames,
    ) -> Result<
        (
            rustc_codegen_ssa::CodegenResults,
            rustc_hash::FxHashMap<
                rustc_middle::dep_graph::WorkProductId,
                rustc_middle::dep_graph::WorkProduct,
            >,
        ),
        rustc_errors::ErrorGuaranteed,
    > {
        unreachable!()
    }

    fn link(
        &self,
        _sess: &rustc_session::Session,
        _codegen_results: rustc_codegen_ssa::CodegenResults,
        _outputs: &rustc_session::config::OutputFilenames,
    ) -> Result<(), rustc_errors::ErrorGuaranteed> {
        unreachable!()
    }
}

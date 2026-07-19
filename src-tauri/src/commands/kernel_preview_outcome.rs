use std::collections::HashMap;

use crate::{
    kernel::preview_projection::{
        CanvasPatch, PreviewHtmlAttributesExecutionOutcome, PreviewHtmlAttributesExecutionReceipt,
        PreviewHtmlAttributesExecutionStatus, PreviewHtmlDeleteExecutionOutcome,
        PreviewHtmlDeleteExecutionReceipt, PreviewHtmlDeleteExecutionStatus,
        PreviewHtmlDuplicateExecutionOutcome, PreviewHtmlDuplicateExecutionReceipt,
        PreviewHtmlDuplicateExecutionStatus, PreviewHtmlInsertDropExecutionOutcome,
        PreviewHtmlInsertDropExecutionReceipt, PreviewHtmlInsertDropExecutionStatus,
        PreviewHtmlTagExecutionOutcome, PreviewHtmlTagExecutionReceipt,
        PreviewHtmlTagExecutionStatus, PreviewHtmlTextExecutionOutcome,
        PreviewHtmlTextExecutionReceipt, PreviewHtmlTextExecutionStatus,
        PreviewLayerDropExecutionOutcome, PreviewLayerDropExecutionReceipt,
        PreviewLayerDropExecutionStatus, PreviewTeraDeleteExecutionOutcome,
        PreviewTeraDeleteExecutionReceipt, PreviewTeraDeleteExecutionStatus,
        PreviewTeraInsertDropExecutionOutcome, PreviewTeraInsertDropExecutionReceipt,
        PreviewTeraInsertDropExecutionStatus, PreviewTeraMoveDropExecutionOutcome,
        PreviewTeraMoveDropExecutionReceipt, PreviewTeraMoveDropExecutionStatus,
    },
    project_model::model::ProjectModel,
};

pub(super) trait PreviewStructuralCommandOutcome {
    type Receipt;

    fn command_succeeded(&self) -> bool;
    fn after_model_mut(&mut self) -> &mut Option<ProjectModel>;
    fn take_alias_updates(&mut self) -> HashMap<String, String> {
        HashMap::new()
    }
    fn canvas_patch_mut(&mut self) -> Option<&mut CanvasPatch> {
        None
    }
    fn into_receipt(self) -> Self::Receipt;
}

pub(super) fn finalize_preview_structural_outcome<O>(
    outcome: Result<O, String>,
) -> Result<O::Receipt, String>
where
    O: PreviewStructuralCommandOutcome,
{
    outcome.map(|mut outcome| {
        if let Some(canvas_patch) = outcome.canvas_patch_mut() {
            canvas_patch.mark_issued_now();
        }
        outcome.into_receipt()
    })
}

impl PreviewStructuralCommandOutcome for PreviewLayerDropExecutionOutcome {
    type Receipt = PreviewLayerDropExecutionReceipt;

    fn command_succeeded(&self) -> bool {
        self.receipt.status == PreviewLayerDropExecutionStatus::Committed
    }

    fn after_model_mut(&mut self) -> &mut Option<ProjectModel> {
        &mut self.after_model
    }

    fn take_alias_updates(&mut self) -> HashMap<String, String> {
        std::mem::take(&mut self.alias_updates)
    }

    fn canvas_patch_mut(&mut self) -> Option<&mut CanvasPatch> {
        self.receipt.canvas_patch.as_mut()
    }

    fn into_receipt(self) -> Self::Receipt {
        self.receipt
    }
}

macro_rules! preview_structural_outcome {
    ($outcome:ty, $receipt:ty, $status:ty) => {
        impl PreviewStructuralCommandOutcome for $outcome {
            type Receipt = $receipt;

            fn command_succeeded(&self) -> bool {
                self.receipt.status == <$status>::Committed
            }

            fn after_model_mut(&mut self) -> &mut Option<ProjectModel> {
                &mut self.after_model
            }

            fn canvas_patch_mut(&mut self) -> Option<&mut CanvasPatch> {
                self.receipt.canvas_patch.as_mut()
            }

            fn into_receipt(self) -> Self::Receipt {
                self.receipt
            }
        }
    };
}

macro_rules! preview_structural_outcome_with_aliases {
    ($outcome:ty, $receipt:ty, $status:ty) => {
        impl PreviewStructuralCommandOutcome for $outcome {
            type Receipt = $receipt;

            fn command_succeeded(&self) -> bool {
                self.receipt.status == <$status>::Committed
            }

            fn after_model_mut(&mut self) -> &mut Option<ProjectModel> {
                &mut self.after_model
            }

            fn take_alias_updates(&mut self) -> HashMap<String, String> {
                std::mem::take(&mut self.alias_updates)
            }

            fn canvas_patch_mut(&mut self) -> Option<&mut CanvasPatch> {
                self.receipt.canvas_patch.as_mut()
            }

            fn into_receipt(self) -> Self::Receipt {
                self.receipt
            }
        }
    };
}

preview_structural_outcome_with_aliases!(
    PreviewHtmlInsertDropExecutionOutcome,
    PreviewHtmlInsertDropExecutionReceipt,
    PreviewHtmlInsertDropExecutionStatus
);
preview_structural_outcome_with_aliases!(
    PreviewHtmlAttributesExecutionOutcome,
    PreviewHtmlAttributesExecutionReceipt,
    PreviewHtmlAttributesExecutionStatus
);
preview_structural_outcome_with_aliases!(
    PreviewHtmlTextExecutionOutcome,
    PreviewHtmlTextExecutionReceipt,
    PreviewHtmlTextExecutionStatus
);
preview_structural_outcome_with_aliases!(
    PreviewHtmlTagExecutionOutcome,
    PreviewHtmlTagExecutionReceipt,
    PreviewHtmlTagExecutionStatus
);
preview_structural_outcome_with_aliases!(
    PreviewHtmlDuplicateExecutionOutcome,
    PreviewHtmlDuplicateExecutionReceipt,
    PreviewHtmlDuplicateExecutionStatus
);
preview_structural_outcome_with_aliases!(
    PreviewHtmlDeleteExecutionOutcome,
    PreviewHtmlDeleteExecutionReceipt,
    PreviewHtmlDeleteExecutionStatus
);
preview_structural_outcome!(
    PreviewTeraInsertDropExecutionOutcome,
    PreviewTeraInsertDropExecutionReceipt,
    PreviewTeraInsertDropExecutionStatus
);
preview_structural_outcome!(
    PreviewTeraMoveDropExecutionOutcome,
    PreviewTeraMoveDropExecutionReceipt,
    PreviewTeraMoveDropExecutionStatus
);
preview_structural_outcome!(
    PreviewTeraDeleteExecutionOutcome,
    PreviewTeraDeleteExecutionReceipt,
    PreviewTeraDeleteExecutionStatus
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::preview_projection::{
        CanvasPatchAnchor, CanvasPatchOperation, CANVAS_PATCH_SCHEMA_VERSION,
    };

    struct TestOutcome {
        canvas_patch: Option<CanvasPatch>,
        after_model: Option<ProjectModel>,
    }

    impl PreviewStructuralCommandOutcome for TestOutcome {
        type Receipt = CanvasPatch;

        fn command_succeeded(&self) -> bool {
            true
        }

        fn after_model_mut(&mut self) -> &mut Option<ProjectModel> {
            &mut self.after_model
        }

        fn canvas_patch_mut(&mut self) -> Option<&mut CanvasPatch> {
            self.canvas_patch.as_mut()
        }

        fn into_receipt(self) -> Self::Receipt {
            self.canvas_patch.expect("test CanvasPatch")
        }
    }

    #[test]
    fn finalization_starts_canvas_delivery_clock_after_workspace_persistence() {
        let patch_id = "canvas_patch_test".to_string();
        let outcome = TestOutcome {
            canvas_patch: Some(CanvasPatch {
                schema_version: CANVAS_PATCH_SCHEMA_VERSION,
                patch_id: patch_id.clone(),
                issued_at_ms: 1,
                project_root: "/project".to_string(),
                runtime_session_id: "runtime".to_string(),
                base_workspace_revision: 4,
                workspace_revision: 5,
                workspace_transaction_id: "workspace-5".to_string(),
                before_model_revision: "before".to_string(),
                after_model_revision: "after".to_string(),
                operation: CanvasPatchOperation::Delete {
                    target: CanvasPatchAnchor::source("sg_0123456789abcdef", None, Some("div")),
                },
            }),
            after_model: None,
        };

        let receipt = finalize_preview_structural_outcome(Ok(outcome)).unwrap();
        assert_eq!(receipt.patch_id, patch_id);
        assert!(receipt.issued_at_ms > 1);
    }
}

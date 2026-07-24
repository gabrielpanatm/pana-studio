use std::path::Path;

use crate::{
    kernel::project_workspace::ProjectWorkspace,
    project_model::{
        attribute_engine::{ProjectHtmlAttributePatch, ProjectHtmlAttributePlan},
        delete_engine::{ProjectHtmlDeletePatch, ProjectHtmlDeletePlan},
        duplicate_engine::{ProjectHtmlDuplicatePatch, ProjectHtmlDuplicatePlan},
        insert_engine::{ProjectHtmlInsertPatch, ProjectHtmlInsertPlan},
        model::ProjectModel,
        move_engine::{ProjectHtmlMovePatch, ProjectHtmlMovePlan},
        tag_engine::{ProjectHtmlTagPatch, ProjectHtmlTagPlan},
        tera_delete_engine::{ProjectTeraDeletePatch, ProjectTeraDeletePlan},
        tera_insert_engine::{ProjectTeraInsertPatch, ProjectTeraInsertPlan},
        tera_move_engine::{ProjectTeraMovePatch, ProjectTeraMovePlan},
        text_engine::{ProjectHtmlTextPatch, ProjectHtmlTextPlan},
    },
};

use super::{
    super::{
        model::PreviewProjectionDiagnostic,
        structural_write::{
            stage_preview_structural_write, PreviewStructuralWrite, PreviewStructuralWriteCommit,
        },
    },
    spec::PreviewStructuralPlanSpec,
};

pub(super) struct PreviewStructuralPlanBlocked {
    pub(super) model_revision: String,
    pub(super) diagnostic: PreviewProjectionDiagnostic,
}

pub(super) struct PreviewStructuralPlanCommitted<P> {
    pub(super) before_model: ProjectModel,
    pub(super) patch: P,
    pub(super) commit: PreviewStructuralWriteCommit,
}

pub(super) fn run_preview_structural_plan<P, Plan>(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    spec: PreviewStructuralPlanSpec,
    plan: impl FnOnce(&ProjectModel) -> Plan,
) -> Result<Result<PreviewStructuralPlanCommitted<P>, PreviewStructuralPlanBlocked>, String>
where
    P: PreviewStructuralPatch,
    Plan: PreviewStructuralPlan<Patch = P>,
{
    run_preview_structural_plan_in_history_group(project_root, workspace, spec, None, plan)
}

pub(super) fn run_preview_structural_plan_in_history_group<P, Plan>(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    spec: PreviewStructuralPlanSpec,
    history_group_id: Option<&str>,
    plan: impl FnOnce(&ProjectModel) -> Plan,
) -> Result<Result<PreviewStructuralPlanCommitted<P>, PreviewStructuralPlanBlocked>, String>
where
    P: PreviewStructuralPatch,
    Plan: PreviewStructuralPlan<Patch = P>,
{
    let projection = workspace.capture_projection_lease()?;
    let before_model = crate::project_model::build_project_model_from_workspace_projection(
        project_root,
        &projection,
    )?;
    let mut patch = match structural_plan_patch_or_block(plan(&before_model), spec) {
        Ok(patch) => patch,
        Err(blocked) => return Ok(Err(blocked)),
    };

    let coalesce_key = match (patch.coalesce_key(), history_group_id) {
        (Some(base), Some(group)) => {
            let group = group.trim();
            if group.is_empty()
                || group.len() > 128
                || !group
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err("Preview HTML text a refuzat editSessionId invalid.".to_string());
            }
            Some(format!("preview.html.text.group:{group}:{base}"))
        }
        (key, None) => key,
        (None, Some(_)) => {
            return Err("Mutația grupată nu are cheie History proiectabilă.".to_string())
        }
    };
    let commit = stage_preview_structural_write(
        project_root,
        workspace,
        PreviewStructuralWrite::new(
            spec.write_label,
            patch.file().to_string(),
            patch.contents().to_string(),
        )
        .with_coalesce_key(coalesce_key),
    )?;
    if patch.contents() != commit.primary_contents {
        patch.replace_authoritative_contents(commit.primary_contents.clone());
    }

    Ok(Ok(PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    }))
}

fn structural_plan_patch_or_block<P, Plan>(
    plan: Plan,
    spec: PreviewStructuralPlanSpec,
) -> Result<P, PreviewStructuralPlanBlocked>
where
    P: PreviewStructuralPatch,
    Plan: PreviewStructuralPlan<Patch = P>,
{
    let (model_revision, diagnostic, patch) = plan.into_parts();
    patch.ok_or_else(|| PreviewStructuralPlanBlocked {
        model_revision,
        diagnostic: PreviewProjectionDiagnostic::blocking(
            spec.blocked_code,
            diagnostic.unwrap_or_else(|| spec.blocked_fallback.to_string()),
        ),
    })
}

pub(super) trait PreviewStructuralPatch {
    fn file(&self) -> &str;
    fn contents(&self) -> &str;

    fn coalesce_key(&self) -> Option<String> {
        None
    }

    fn replace_authoritative_contents(&mut self, contents: String);
}

pub(super) trait PreviewStructuralPlan {
    type Patch: PreviewStructuralPatch;

    fn into_parts(self) -> (String, Option<String>, Option<Self::Patch>);
}

macro_rules! preview_structural_patch {
    ($patch:ty) => {
        impl PreviewStructuralPatch for $patch {
            fn file(&self) -> &str {
                &self.file
            }

            fn contents(&self) -> &str {
                &self.contents
            }

            fn replace_authoritative_contents(&mut self, contents: String) {
                self.after_revision =
                    crate::project_model::move_engine::content_revision(&contents);
                self.contents = contents;
            }
        }
    };
}

macro_rules! preview_structural_plan {
    ($plan:ty, $patch:ty) => {
        impl PreviewStructuralPlan for $plan {
            type Patch = $patch;

            fn into_parts(self) -> (String, Option<String>, Option<Self::Patch>) {
                (self.model_revision, self.diagnostic, self.patch)
            }
        }
    };
}

preview_structural_patch!(ProjectHtmlMovePatch);
preview_structural_patch!(ProjectHtmlInsertPatch);
preview_structural_patch!(ProjectHtmlTagPatch);
preview_structural_patch!(ProjectHtmlDuplicatePatch);
preview_structural_patch!(ProjectHtmlDeletePatch);
preview_structural_patch!(ProjectTeraInsertPatch);
preview_structural_patch!(ProjectTeraMovePatch);
preview_structural_patch!(ProjectTeraDeletePatch);

impl PreviewStructuralPatch for ProjectHtmlAttributePatch {
    fn file(&self) -> &str {
        &self.file
    }

    fn contents(&self) -> &str {
        &self.contents
    }

    fn coalesce_key(&self) -> Option<String> {
        Some(format!(
            "preview.html.attributes:{}:{}",
            self.file, self.resolved_target_id
        ))
    }

    fn replace_authoritative_contents(&mut self, contents: String) {
        self.after_revision = crate::project_model::move_engine::content_revision(&contents);
        self.contents = contents;
    }
}

impl PreviewStructuralPatch for ProjectHtmlTextPatch {
    fn file(&self) -> &str {
        &self.file
    }

    fn contents(&self) -> &str {
        &self.contents
    }

    fn coalesce_key(&self) -> Option<String> {
        Some(format!(
            "preview.html.text:{}:{}",
            self.file, self.resolved_target_id
        ))
    }

    fn replace_authoritative_contents(&mut self, contents: String) {
        self.after_revision = crate::project_model::move_engine::content_revision(&contents);
        self.contents = contents;
    }
}

preview_structural_plan!(ProjectHtmlMovePlan, ProjectHtmlMovePatch);
preview_structural_plan!(ProjectHtmlInsertPlan, ProjectHtmlInsertPatch);
preview_structural_plan!(ProjectHtmlAttributePlan, ProjectHtmlAttributePatch);
preview_structural_plan!(ProjectHtmlTextPlan, ProjectHtmlTextPatch);
preview_structural_plan!(ProjectHtmlTagPlan, ProjectHtmlTagPatch);
preview_structural_plan!(ProjectHtmlDuplicatePlan, ProjectHtmlDuplicatePatch);
preview_structural_plan!(ProjectHtmlDeletePlan, ProjectHtmlDeletePatch);
preview_structural_plan!(ProjectTeraInsertPlan, ProjectTeraInsertPatch);
preview_structural_plan!(ProjectTeraMovePlan, ProjectTeraMovePatch);
preview_structural_plan!(ProjectTeraDeletePlan, ProjectTeraDeletePatch);

#[cfg(test)]
mod tests {
    use super::super::spec::{HTML_INSERT_DROP_PLAN, LAYER_DROP_PLAN};
    use super::*;

    #[test]
    fn structural_plan_patch_or_block_uses_plan_diagnostic_when_missing_patch() {
        let plan = ProjectHtmlMovePlan {
            allowed: false,
            diagnostic: Some("Ancora nu mai există.".to_string()),
            model_revision: "model-1".to_string(),
            patch: None,
        };

        let blocked =
            structural_plan_patch_or_block::<ProjectHtmlMovePatch, _>(plan, LAYER_DROP_PLAN)
                .expect_err("plan fără patch trebuie blocat");

        assert_eq!(blocked.model_revision, "model-1");
        assert_eq!(
            blocked.diagnostic.code,
            "preview_layer_drop_move_plan_blocked"
        );
        assert_eq!(blocked.diagnostic.message, "Ancora nu mai există.");
        assert!(blocked.diagnostic.blocking);
    }

    #[test]
    fn structural_plan_patch_or_block_uses_fallback_when_plan_has_no_diagnostic() {
        let plan = ProjectHtmlInsertPlan {
            allowed: false,
            diagnostic: None,
            model_revision: "model-2".to_string(),
            patch: None,
        };

        let blocked = structural_plan_patch_or_block::<ProjectHtmlInsertPatch, _>(
            plan,
            HTML_INSERT_DROP_PLAN,
        )
        .expect_err("plan fără patch trebuie blocat");

        assert_eq!(blocked.model_revision, "model-2");
        assert_eq!(
            blocked.diagnostic.code,
            "preview_html_insert_drop_plan_blocked"
        );
        assert_eq!(
            blocked.diagnostic.message,
            HTML_INSERT_DROP_PLAN.blocked_fallback
        );
        assert!(blocked.diagnostic.blocking);
    }
}

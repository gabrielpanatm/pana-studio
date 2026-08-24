use std::{collections::HashSet, path::Path, sync::Arc};

use crate::{
    kernel::project_workspace::ProjectWorkspace,
    localization::LocalizedDiagnostic,
    project_model::cache::{
        project_model_sources_match_projection, rebuild_project_model_from_previous_projection,
    },
    project_model::{
        attribute_engine::{ProjectHtmlAttributePatch, ProjectHtmlAttributePlan},
        delete_engine::{ProjectHtmlDeletePatch, ProjectHtmlDeletePlan},
        duplicate_engine::{ProjectHtmlDuplicatePatch, ProjectHtmlDuplicatePlan},
        insert_engine::{ProjectHtmlInsertPatch, ProjectHtmlInsertPlan},
        model::{ProjectModel, ProjectModelFileKind},
        move_engine::{ProjectHtmlMovePatch, ProjectHtmlMovePlan, ProjectMovePosition},
        tag_engine::{ProjectHtmlTagPatch, ProjectHtmlTagPlan},
        tera_delete_engine::{ProjectTeraDeletePatch, ProjectTeraDeletePlan},
        tera_insert_engine::{ProjectTeraInsertPatch, ProjectTeraInsertPlan},
        tera_move_engine::{ProjectTeraMovePatch, ProjectTeraMovePlan},
        text_engine::{ProjectHtmlTextPatch, ProjectHtmlTextPlan},
        ProjectModelIncrementalIntent,
    },
    source_graph::identity::{SourceChangeSet, SourceTreeMovePosition},
};

use super::{
    super::{
        model::PreviewProjectionDiagnostic,
        structural_write::{
            stage_preview_structural_write_in_transaction, PreviewStructuralWrite,
            PreviewStructuralWriteCommit,
        },
    },
    spec::PreviewStructuralPlanSpec,
};

pub(super) struct PreviewStructuralPlanBlocked {
    pub(super) model_revision: String,
    pub(super) diagnostic: PreviewProjectionDiagnostic,
}

pub(super) struct PreviewStructuralPlanCommitted<P> {
    pub(super) before_model: Arc<ProjectModel>,
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

pub(super) fn run_preview_structural_plan_with_model<P, Plan>(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: Arc<ProjectModel>,
    spec: PreviewStructuralPlanSpec,
    plan: impl FnOnce(&ProjectModel) -> Plan,
) -> Result<Result<PreviewStructuralPlanCommitted<P>, PreviewStructuralPlanBlocked>, String>
where
    P: PreviewStructuralPatch,
    Plan: PreviewStructuralPlan<Patch = P>,
{
    let before_model =
        require_authoritative_structural_model(project_root, workspace, before_model)?;
    run_preview_structural_plan_with_model_in_history_group(
        project_root,
        workspace,
        before_model,
        spec,
        None,
        plan,
    )
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
    let projection = workspace.capture_projection_snapshot()?;
    let cached_model = workspace.project_model.clone();
    let before_model = if workspace.project_model_source_revision == Some(projection.revision)
        && cached_model
            .as_ref()
            .is_some_and(|model| project_model_sources_match_projection(model, &projection))
    {
        cached_model.ok_or_else(|| {
            "ProjectWorkspace declară o revizie ProjectModel curentă fără modelul canonic."
                .to_string()
        })?
    } else {
        let model = rebuild_project_model_from_previous_projection(
            project_root,
            cached_model.as_ref(),
            workspace.project_model_source_revision,
            &projection,
        )?;
        workspace.publish_project_model(&projection, model.clone())?;
        model
    };
    run_preview_structural_plan_with_model_in_history_group(
        project_root,
        workspace,
        before_model,
        spec,
        history_group_id,
        plan,
    )
}

fn require_authoritative_structural_model(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    candidate: Arc<ProjectModel>,
) -> Result<Arc<ProjectModel>, String> {
    let projection = workspace.capture_projection_snapshot()?;
    if project_model_sources_match_projection(&candidate, &projection) {
        return Ok(candidate);
    }
    let model = rebuild_project_model_from_previous_projection(
        project_root,
        Some(&candidate),
        workspace.project_model_source_revision,
        &projection,
    )?;
    workspace.publish_project_model(&projection, model.clone())?;
    Ok(model)
}

fn run_preview_structural_plan_with_model_in_history_group<P, Plan>(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: Arc<ProjectModel>,
    spec: PreviewStructuralPlanSpec,
    history_group_id: Option<&str>,
    plan: impl FnOnce(&ProjectModel) -> Plan,
) -> Result<Result<PreviewStructuralPlanCommitted<P>, PreviewStructuralPlanBlocked>, String>
where
    P: PreviewStructuralPatch,
    Plan: PreviewStructuralPlan<Patch = P>,
{
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
    let source_changes = patch.source_changes(&before_model)?;
    let commit = stage_preview_structural_write_in_transaction(
        project_root,
        workspace,
        PreviewStructuralWrite::new(
            spec.write_label,
            patch.file().to_string(),
            patch.contents().to_string(),
        )
        .with_coalesce_key(coalesce_key)
        .with_project_model_incremental_intent(patch.project_model_incremental_intent())
        .with_source_changes(source_changes),
    )?;
    if patch.contents() != commit.primary_contents {
        patch.replace_authoritative_contents(commit.primary_contents.clone());
    }
    validate_structural_projection(
        &before_model,
        &commit.after_model,
        patch.file(),
        patch.contents(),
    )?;
    let operation_model = commit
        .intermediate_model
        .as_ref()
        .unwrap_or(&commit.after_model);
    patch.validate_after_model(&before_model, operation_model)?;

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
            diagnostic
                .filter(|details| !details.trim().is_empty())
                .map(|details| {
                    LocalizedDiagnostic::new(
                        "preview-projection-structural-plan-blocked-with-details",
                    )
                    .with_argument("details", details)
                })
                .unwrap_or_else(|| {
                    LocalizedDiagnostic::new("preview-projection-structural-plan-blocked")
                }),
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

    fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
        ProjectModelIncrementalIntent::Unsupported
    }

    fn source_changes(
        &self,
        before_model: &ProjectModel,
    ) -> Result<Option<Vec<SourceChangeSet>>, String> {
        let before = before_model
            .files
            .iter()
            .find(|file| same_model_file(&file.relative_path, self.file()))
            .ok_or_else(|| {
                format!(
                    "SourceChangeSet nu a găsit sursa autoritativă {} înainte de mutație.",
                    self.file()
                )
            })?;
        Ok(Some(vec![SourceChangeSet::between(
            self.file(),
            &before.contents,
            self.contents(),
        )]))
    }

    fn validate_after_model(
        &self,
        _before_model: &ProjectModel,
        _after_model: &ProjectModel,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub(super) trait PreviewStructuralPlan {
    type Patch: PreviewStructuralPatch;

    fn into_parts(self) -> (String, Option<String>, Option<Self::Patch>);
}

macro_rules! preview_structural_patch {
    ($patch:ty, $intent:expr) => {
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

            fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
                $intent
            }
        }
    };
}

macro_rules! preview_delete_structural_patch {
    ($patch:ty, $validate:ident) => {
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

            fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
                ProjectModelIncrementalIntent::HtmlStructural
            }

            fn source_changes(
                &self,
                before_model: &ProjectModel,
            ) -> Result<Option<Vec<SourceChangeSet>>, String> {
                let before = before_model
                    .files
                    .iter()
                    .find(|file| same_model_file(&file.relative_path, &self.file))
                    .ok_or_else(|| {
                        format!(
                            "SourceChangeSet nu a găsit sursa autoritativă {} înainte de ștergere.",
                            self.file
                        )
                    })?;
                Ok(Some(vec![SourceChangeSet::between(
                    &self.file,
                    &before.contents,
                    &self.contents,
                )
                .with_tree_delete(&self.resolved_target_id)]))
            }

            fn validate_after_model(
                &self,
                before_model: &ProjectModel,
                after_model: &ProjectModel,
            ) -> Result<(), String> {
                $validate(before_model, after_model, self)
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

impl PreviewStructuralPatch for ProjectHtmlMovePatch {
    fn file(&self) -> &str {
        &self.file
    }

    fn contents(&self) -> &str {
        &self.contents
    }

    fn replace_authoritative_contents(&mut self, contents: String) {
        self.after_revision = crate::project_model::move_engine::content_revision(&contents);
        self.contents = contents;
    }

    fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
        ProjectModelIncrementalIntent::HtmlStructural
    }

    fn source_changes(
        &self,
        before_model: &ProjectModel,
    ) -> Result<Option<Vec<SourceChangeSet>>, String> {
        let before = before_model
            .files
            .iter()
            .find(|file| same_model_file(&file.relative_path, &self.file))
            .ok_or_else(|| {
                format!(
                    "SourceChangeSet nu a găsit sursa autoritativă {} înainte de mutare.",
                    self.file
                )
            })?;
        let position = match self.position {
            ProjectMovePosition::Before => SourceTreeMovePosition::Before,
            ProjectMovePosition::After => SourceTreeMovePosition::After,
            ProjectMovePosition::Inside => SourceTreeMovePosition::Inside,
        };
        Ok(Some(vec![SourceChangeSet::between(
            &self.file,
            &before.contents,
            &self.contents,
        )
        .with_tree_move(
            &self.resolved_source_id,
            &self.resolved_target_id,
            position,
        )]))
    }

    fn validate_after_model(
        &self,
        before_model: &ProjectModel,
        after_model: &ProjectModel,
    ) -> Result<(), String> {
        validate_html_move_after_model(before_model, after_model, self)
    }
}

impl PreviewStructuralPatch for ProjectHtmlDuplicatePatch {
    fn file(&self) -> &str {
        &self.file
    }

    fn contents(&self) -> &str {
        &self.contents
    }

    fn replace_authoritative_contents(&mut self, contents: String) {
        self.after_revision = crate::project_model::move_engine::content_revision(&contents);
        self.contents = contents;
    }

    fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
        ProjectModelIncrementalIntent::HtmlStructural
    }

    fn source_changes(
        &self,
        before_model: &ProjectModel,
    ) -> Result<Option<Vec<SourceChangeSet>>, String> {
        let before = before_model
            .files
            .iter()
            .find(|file| same_model_file(&file.relative_path, &self.file))
            .ok_or_else(|| {
                format!(
                    "SourceChangeSet nu a găsit sursa autoritativă {} înainte de duplicare.",
                    self.file
                )
            })?;
        Ok(Some(vec![SourceChangeSet::between(
            &self.file,
            &before.contents,
            &self.contents,
        )
        .with_tree_duplicate(
            &self.resolved_source_id,
            self.inserted_offset,
        )]))
    }

    fn validate_after_model(
        &self,
        before_model: &ProjectModel,
        after_model: &ProjectModel,
    ) -> Result<(), String> {
        validate_html_duplicate_after_model(before_model, after_model, self)
    }
}

impl PreviewStructuralPatch for ProjectTeraMovePatch {
    fn file(&self) -> &str {
        &self.file
    }

    fn contents(&self) -> &str {
        &self.contents
    }

    fn replace_authoritative_contents(&mut self, contents: String) {
        self.after_revision = crate::project_model::move_engine::content_revision(&contents);
        self.contents = contents;
    }

    fn source_changes(
        &self,
        before_model: &ProjectModel,
    ) -> Result<Option<Vec<SourceChangeSet>>, String> {
        let before = before_model
            .files
            .iter()
            .find(|file| same_model_file(&file.relative_path, &self.file))
            .ok_or_else(|| {
                format!(
                    "SourceChangeSet nu a găsit sursa autoritativă {} înainte de mutarea Tera.",
                    self.file
                )
            })?;
        let position = match self.position {
            ProjectMovePosition::Before => SourceTreeMovePosition::Before,
            ProjectMovePosition::After => SourceTreeMovePosition::After,
            ProjectMovePosition::Inside => SourceTreeMovePosition::Inside,
        };
        Ok(Some(vec![SourceChangeSet::between(
            &self.file,
            &before.contents,
            &self.contents,
        )
        .with_tree_move(
            &self.resolved_source_id,
            &self.resolved_target_id,
            position,
        )]))
    }

    fn validate_after_model(
        &self,
        before_model: &ProjectModel,
        after_model: &ProjectModel,
    ) -> Result<(), String> {
        validate_tera_move_after_model(before_model, after_model, self)
    }
}
impl PreviewStructuralPatch for ProjectHtmlInsertPatch {
    fn file(&self) -> &str {
        &self.file
    }

    fn contents(&self) -> &str {
        &self.contents
    }

    fn replace_authoritative_contents(&mut self, contents: String) {
        self.after_revision = crate::project_model::move_engine::content_revision(&contents);
        self.contents = contents;
    }

    fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
        ProjectModelIncrementalIntent::HtmlStructural
    }

    fn source_changes(
        &self,
        before_model: &ProjectModel,
    ) -> Result<Option<Vec<SourceChangeSet>>, String> {
        let before = before_model
            .files
            .iter()
            .find(|file| same_model_file(&file.relative_path, &self.file))
            .ok_or_else(|| {
                format!(
                    "SourceChangeSet nu a găsit sursa autoritativă {} înainte de inserarea HTML.",
                    self.file
                )
            })?;
        let position = match self.position {
            ProjectMovePosition::Before => SourceTreeMovePosition::Before,
            ProjectMovePosition::After => SourceTreeMovePosition::After,
            ProjectMovePosition::Inside => SourceTreeMovePosition::Inside,
        };
        let mut change = SourceChangeSet::between(&self.file, &before.contents, &self.contents);
        if let Some(edit) = self.exact_source_edit() {
            change = change.with_exact_text_edits(vec![edit]);
        }
        Ok(Some(vec![change.with_tree_insert(
            &self.resolved_target_id,
            position,
            self.inside_child_index(),
            self.inserted_offset(),
        )]))
    }

    fn validate_after_model(
        &self,
        before_model: &ProjectModel,
        after_model: &ProjectModel,
    ) -> Result<(), String> {
        validate_html_insert_after_model(before_model, after_model, self)
    }
}
preview_structural_patch!(
    ProjectHtmlTagPatch,
    ProjectModelIncrementalIntent::HtmlStructural
);
preview_delete_structural_patch!(ProjectHtmlDeletePatch, validate_html_delete_after_model);
impl PreviewStructuralPatch for ProjectTeraInsertPatch {
    fn file(&self) -> &str {
        &self.file
    }

    fn contents(&self) -> &str {
        &self.contents
    }

    fn replace_authoritative_contents(&mut self, contents: String) {
        self.after_revision = crate::project_model::move_engine::content_revision(&contents);
        self.contents = contents;
    }

    fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
        ProjectModelIncrementalIntent::HtmlStructural
    }

    fn source_changes(
        &self,
        before_model: &ProjectModel,
    ) -> Result<Option<Vec<SourceChangeSet>>, String> {
        let before = before_model
            .files
            .iter()
            .find(|file| same_model_file(&file.relative_path, &self.file))
            .ok_or_else(|| {
                format!(
                    "SourceChangeSet nu a găsit sursa autoritativă {} înainte de inserarea Tera.",
                    self.file
                )
            })?;
        let position = match self.position {
            ProjectMovePosition::Before => SourceTreeMovePosition::Before,
            ProjectMovePosition::After => SourceTreeMovePosition::After,
            ProjectMovePosition::Inside => SourceTreeMovePosition::Inside,
        };
        Ok(Some(vec![SourceChangeSet::between(
            &self.file,
            &before.contents,
            &self.contents,
        )
        .with_exact_text_edits(vec![self.exact_source_edit()])
        .with_tree_insert(
            &self.resolved_target_id,
            position,
            self.expected_child_index,
            Some(self.inserted_offset()),
        )]))
    }

    fn validate_after_model(
        &self,
        before_model: &ProjectModel,
        after_model: &ProjectModel,
    ) -> Result<(), String> {
        validate_tera_insert_after_model(before_model, after_model, self)
    }
}
preview_delete_structural_patch!(ProjectTeraDeletePatch, validate_tera_delete_after_model);

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

    fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
        ProjectModelIncrementalIntent::HtmlStructural
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

    fn project_model_incremental_intent(&self) -> ProjectModelIncrementalIntent {
        ProjectModelIncrementalIntent::HtmlStructural
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

fn validate_structural_projection(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    file: &str,
    authoritative_contents: &str,
) -> Result<(), String> {
    let projected_file = after_model
        .files
        .iter()
        .find(|candidate| same_model_file(&candidate.relative_path, file))
        .ok_or_else(|| {
            format!("Postcondiția structurală nu a regăsit fișierul {file} în after_model.")
        })?;
    if projected_file.contents != authoritative_contents {
        return Err(format!(
            "Postcondiția structurală a detectat conținut divergent în after_model pentru {file}."
        ));
    }

    if projected_file.kind == ProjectModelFileKind::Template
        || projected_file.relative_path.ends_with(".html")
    {
        let document = crate::source_graph::mixed_cst::parse_mixed_cst(
            &projected_file.contents,
            &projected_file.relative_path,
        );
        if !document.is_lossless() {
            return Err(format!(
                "Postcondiția structurală Mixed CST nu este lossless pentru {file}."
            ));
        }
        if !document.tera.is_valid_tera() {
            return Err(format!(
                "Postcondiția structurală a detectat sintaxă Tera invalidă în {file}."
            ));
        }
    }

    validate_source_graph_consistency(after_model)?;
    let introduced_invalid_native_contract = after_model
        .source_graph
        .block_graph
        .source_instances
        .iter()
        .filter(|instance| same_model_file(&instance.file, file))
        .filter(|instance| {
            instance.status == crate::source_graph::model::BlockResolutionStatus::InvalidContract
        })
        .any(|after_instance| {
            !before_model
                .source_graph
                .block_graph
                .source_instances
                .iter()
                .any(|before_instance| {
                    before_instance.id == after_instance.id
                        && before_instance.status
                            == crate::source_graph::model::BlockResolutionStatus::InvalidContract
                })
        });
    if introduced_invalid_native_contract {
        return Err(format!(
            "Postcondiția structurală a introdus un contract de bloc nativ invalid în {file}."
        ));
    }
    Ok(())
}

fn validate_source_graph_consistency(model: &ProjectModel) -> Result<(), String> {
    let mut ids = HashSet::with_capacity(model.source_graph.nodes.len());
    for node in &model.source_graph.nodes {
        if !ids.insert(node.id.as_str()) {
            return Err(format!(
                "Postcondiția Source Graph a găsit ID duplicat: {}.",
                node.id
            ));
        }
    }

    for node in &model.source_graph.nodes {
        if let Some(parent_id) = node.parent.as_deref() {
            let parent = model.source_graph.node_by_id(parent_id).ok_or_else(|| {
                format!(
                    "Postcondiția Source Graph nu a găsit părintele {parent_id} pentru {}.",
                    node.id
                )
            })?;
            if !parent.children.iter().any(|child| child == &node.id) {
                return Err(format!(
                    "Postcondiția Source Graph nu găsește {} între copiii părintelui {parent_id}.",
                    node.id
                ));
            }
        }

        let mut children = HashSet::with_capacity(node.children.len());
        for child_id in &node.children {
            if !children.insert(child_id.as_str()) {
                return Err(format!(
                    "Postcondiția Source Graph a găsit copilul {child_id} duplicat în {}.",
                    node.id
                ));
            }
            let child = model.source_graph.node_by_id(child_id).ok_or_else(|| {
                format!(
                    "Postcondiția Source Graph nu a găsit copilul {child_id} al nodului {}.",
                    node.id
                )
            })?;
            if child.parent.as_deref() != Some(node.id.as_str()) {
                return Err(format!(
                    "Postcondiția Source Graph a găsit o relație nereciprocă între {} și {child_id}.",
                    node.id
                ));
            }
        }

        if let Some(range) = node.range.as_ref() {
            let source_file = model
                .files
                .iter()
                .find(|file| same_model_file(&file.relative_path, &node.file))
                .ok_or_else(|| {
                    format!(
                        "Postcondiția Source Graph nu a găsit fișierul {} pentru nodul {}.",
                        node.file, node.id
                    )
                })?;
            if range.start > range.end
                || range.end > source_file.contents.len()
                || !source_file.contents.is_char_boundary(range.start)
                || !source_file.contents.is_char_boundary(range.end)
            {
                return Err(format!(
                    "Postcondiția Source Graph a găsit un range invalid pentru {}.",
                    node.id
                ));
            }
        }
    }
    Ok(())
}

fn validate_html_move_after_model(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &ProjectHtmlMovePatch,
) -> Result<(), String> {
    let source = node_by_id(after_model, &patch.resolved_source_id, "sursa mutată")?;
    let target = node_by_id(after_model, &patch.resolved_target_id, "destinația mutării")?;
    validate_structural_relation(after_model, source, target, patch.position, "mutare")?;
    let before_source = node_by_id(before_model, &patch.resolved_source_id, "sursa mutării")?;
    let before_target = node_by_id(before_model, &patch.resolved_target_id, "ținta mutării")?;
    validate_untouched_bytes(
        before_model,
        after_model,
        &patch.file,
        &[before_source, before_target],
        &[source, target],
        "mutare",
    )
}

fn validate_tera_move_after_model(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &ProjectTeraMovePatch,
) -> Result<(), String> {
    let source = node_by_id(after_model, &patch.resolved_source_id, "sursa Tera mutată")?;
    let target = node_by_id(
        after_model,
        &patch.resolved_target_id,
        "destinația mutării Tera",
    )?;
    validate_tera_relation(
        after_model,
        source,
        target,
        patch.position,
        patch.expected_child_index,
        "mutare Tera",
    )?;
    let before_source = node_by_id(
        before_model,
        &patch.resolved_source_id,
        "sursa mutării Tera",
    )?;
    let before_target = node_by_id(
        before_model,
        &patch.resolved_target_id,
        "ținta mutării Tera",
    )?;
    validate_untouched_bytes(
        before_model,
        after_model,
        &patch.file,
        &[before_source, before_target],
        &[source, target],
        "mutare Tera",
    )
}

fn validate_tera_insert_after_model(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &ProjectTeraInsertPatch,
) -> Result<(), String> {
    let inserted = inserted_tera_source_nodes(before_model, after_model, patch)?;
    let target = node_by_id(
        after_model,
        &patch.resolved_target_id,
        "destinația inserării Tera",
    )?;
    let before_target = node_by_id(
        before_model,
        &patch.resolved_target_id,
        "ținta inserării Tera",
    )?;
    let mut after_envelopes = inserted;
    after_envelopes.push(target);
    validate_untouched_bytes(
        before_model,
        after_model,
        &patch.file,
        &[before_target],
        &after_envelopes,
        "inserare Tera",
    )
}

fn validate_html_delete_after_model(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &ProjectHtmlDeletePatch,
) -> Result<(), String> {
    validate_deleted_source_id(
        before_model,
        after_model,
        &patch.resolved_target_id,
        "ștergerea HTML",
    )
}

fn validate_tera_delete_after_model(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &ProjectTeraDeletePatch,
) -> Result<(), String> {
    validate_deleted_source_id(
        before_model,
        after_model,
        &patch.resolved_target_id,
        "ștergerea Tera",
    )
}

fn validate_deleted_source_id(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    source_node_id: &str,
    operation: &str,
) -> Result<(), String> {
    node_by_id(before_model, source_node_id, "ținta ștergerii")?;
    if after_model
        .source_graph
        .node_by_id(source_node_id)
        .is_some()
    {
        return Err(format!(
            "Postcondiția pentru {operation} a păstrat SourceNodeId-ul șters {source_node_id}."
        ));
    }
    Ok(())
}

pub(super) fn inserted_tera_source_nodes<'a>(
    before_model: &ProjectModel,
    after_model: &'a ProjectModel,
    patch: &ProjectTeraInsertPatch,
) -> Result<Vec<&'a crate::source_graph::model::SourceNode>, String> {
    let inserted_kind = (!matches!(patch.inserted_kind.as_str(), "macroCall" | "dynamicWidget"))
        .then_some(patch.inserted_kind.as_str());
    let target = node_by_id(
        after_model,
        &patch.resolved_target_id,
        "ținta inserării Tera",
    )?;
    let before_ids = before_model
        .source_graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let parent = match patch.position {
        ProjectMovePosition::Inside => target,
        ProjectMovePosition::Before | ProjectMovePosition::After => target
            .parent
            .as_deref()
            .and_then(|parent| after_model.source_graph.node_by_id(parent))
            .ok_or_else(|| {
                "Postcondiția inserării Tera nu a găsit părintele destinației.".to_string()
            })?,
    };
    let inserted_positions = parent
        .children
        .iter()
        .enumerate()
        .filter(|(_, id)| !before_ids.contains(id.as_str()))
        .collect::<Vec<_>>();
    if inserted_positions.is_empty()
        || inserted_positions
            .windows(2)
            .any(|pair| pair[1].0 != pair[0].0 + 1)
    {
        return Err("Postcondiția inserării Tera nu a găsit rădăcini noi contigue.".to_string());
    }
    let first_index = inserted_positions[0].0;
    let last_index = inserted_positions[inserted_positions.len() - 1].0;
    let target_index = parent.children.iter().position(|id| id == &target.id);
    let exact_position = match patch.position {
        ProjectMovePosition::Inside => patch.expected_child_index == Some(first_index),
        ProjectMovePosition::Before => target_index == Some(last_index + 1),
        ProjectMovePosition::After => target_index.is_some_and(|index| first_index == index + 1),
    };
    if !exact_position {
        return Err("Postcondiția inserării Tera a găsit rădăcinile noi la alt index.".to_string());
    }
    let inserted = inserted_positions
        .into_iter()
        .map(|(_, id)| {
            after_model
                .source_graph
                .node_by_id(id)
                .ok_or_else(|| format!("Postcondiția inserării Tera a pierdut rădăcina nouă {id}."))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if inserted
        .iter()
        .any(|node| node.parent.as_deref() != Some(parent.id.as_str()))
    {
        return Err(
            "Postcondiția inserării Tera a găsit o relație părinte-copil divergentă.".to_string(),
        );
    }
    if inserted_kind
        .is_some_and(|kind| inserted.len() != 1 || structural_kind_label(&inserted[0].kind) != kind)
    {
        return Err(
            "Postcondiția inserării Tera a găsit un kind sau număr de rădăcini divergent."
                .to_string(),
        );
    }
    let inserted_ids = inserted
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let unexpected_root = after_model.source_graph.nodes.iter().any(|node| {
        same_model_file(&node.file, &patch.file)
            && !before_ids.contains(node.id.as_str())
            && node
                .parent
                .as_deref()
                .is_some_and(|parent| before_ids.contains(parent))
            && !inserted_ids.contains(node.id.as_str())
    });
    if unexpected_root {
        return Err(
            "Postcondiția inserării Tera a găsit o rădăcină nouă în afara splice-ului.".to_string(),
        );
    }
    Ok(inserted)
}

pub(super) fn inserted_html_source_node<'a>(
    before_model: &ProjectModel,
    after_model: &'a ProjectModel,
    patch: &ProjectHtmlInsertPatch,
) -> Result<&'a crate::source_graph::model::SourceNode, String> {
    let target = node_by_id(after_model, &patch.resolved_target_id, "ținta inserării")?;
    unique_new_source_root(
        before_model,
        after_model,
        &patch.file,
        |node| {
            node.kind == crate::source_graph::model::SourceNodeKind::Html
                && html_label_has_tag(&node.label, &patch.tag)
                && validate_structural_relation(
                    after_model,
                    node,
                    target,
                    patch.position,
                    "inserare",
                )
                .is_ok()
        },
        "elementul HTML inserat",
    )
}

pub(super) fn duplicated_html_source_node<'a>(
    before_model: &ProjectModel,
    after_model: &'a ProjectModel,
    patch: &ProjectHtmlDuplicatePatch,
) -> Result<&'a crate::source_graph::model::SourceNode, String> {
    let source = node_by_id(after_model, &patch.resolved_source_id, "sursa duplicării")?;
    unique_new_source_root(
        before_model,
        after_model,
        &patch.file,
        |node| {
            node.kind == crate::source_graph::model::SourceNodeKind::Html
                && html_label_has_tag(&node.label, &patch.tag)
                && validate_structural_relation(
                    after_model,
                    node,
                    source,
                    ProjectMovePosition::After,
                    "duplicare",
                )
                .is_ok()
        },
        "rădăcina HTML duplicată",
    )
}

fn unique_new_source_root<'a>(
    before_model: &ProjectModel,
    after_model: &'a ProjectModel,
    file: &str,
    predicate: impl Fn(&crate::source_graph::model::SourceNode) -> bool,
    role: &str,
) -> Result<&'a crate::source_graph::model::SourceNode, String> {
    let before_ids = before_model
        .source_graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let candidates = after_model
        .source_graph
        .nodes
        .iter()
        .filter(|node| {
            same_model_file(&node.file, file)
                && !before_ids.contains(node.id.as_str())
                && node
                    .parent
                    .as_deref()
                    .is_none_or(|parent| before_ids.contains(parent))
                && predicate(node)
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [node] => Ok(*node),
        _ => Err(format!(
            "Postcondiția structurală a găsit {} candidați noi pentru {role}; mutația a fost refuzată fail-closed.",
            candidates.len()
        )),
    }
}

pub(super) fn confirmed_html_move_position(
    after_model: &ProjectModel,
    patch: &ProjectHtmlMovePatch,
) -> Result<ProjectMovePosition, String> {
    let source = node_by_id(after_model, &patch.resolved_source_id, "sursa mutată")?;
    let target = node_by_id(after_model, &patch.resolved_target_id, "destinația mutării")?;
    confirmed_html_relation(after_model, source, target, "mutare")
}

fn validate_html_insert_after_model(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &ProjectHtmlInsertPatch,
) -> Result<(), String> {
    let inserted = inserted_html_source_node(before_model, after_model, patch)?;
    let target = node_by_id(
        after_model,
        &patch.resolved_target_id,
        "destinația inserării",
    )?;
    validate_structural_relation(after_model, inserted, target, patch.position, "inserare")?;
    let before_target = node_by_id(before_model, &patch.resolved_target_id, "ținta inserării")?;
    validate_untouched_bytes(
        before_model,
        after_model,
        &patch.file,
        &[before_target],
        &[inserted, target],
        "inserare",
    )
}

pub(super) fn confirmed_html_insert_position(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &ProjectHtmlInsertPatch,
) -> Result<Option<ProjectMovePosition>, String> {
    let inserted = inserted_html_source_node(before_model, after_model, patch)?;
    let target = node_by_id(
        after_model,
        &patch.resolved_target_id,
        "destinația inserării",
    )?;
    confirmed_html_relation(after_model, inserted, target, "inserare").map(Some)
}

fn validate_html_duplicate_after_model(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &ProjectHtmlDuplicatePatch,
) -> Result<(), String> {
    let inserted = duplicated_html_source_node(before_model, after_model, patch)?;
    let source = node_by_id(after_model, &patch.resolved_source_id, "sursa duplicată")?;
    validate_structural_relation(
        after_model,
        inserted,
        source,
        ProjectMovePosition::After,
        "duplicare",
    )?;
    let before_source = node_by_id(before_model, &patch.resolved_source_id, "sursa duplicării")?;
    validate_untouched_bytes(
        before_model,
        after_model,
        &patch.file,
        &[before_source],
        &[source, inserted],
        "duplicare",
    )
}

fn node_by_id<'a>(
    model: &'a ProjectModel,
    id: &str,
    role: &str,
) -> Result<&'a crate::source_graph::model::SourceNode, String> {
    model
        .source_graph
        .node_by_id(id)
        .ok_or_else(|| format!("Postcondiția structurală nu a găsit {role} ({id})."))
}

fn validate_untouched_bytes(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    file: &str,
    before_envelopes: &[&crate::source_graph::model::SourceNode],
    after_envelopes: &[&crate::source_graph::model::SourceNode],
    operation: &str,
) -> Result<(), String> {
    let before = before_model
        .files
        .iter()
        .find(|candidate| same_model_file(&candidate.relative_path, file))
        .ok_or_else(|| format!("Postcondiția {operation} nu a găsit sursa inițială {file}."))?;
    let after = after_model
        .files
        .iter()
        .find(|candidate| same_model_file(&candidate.relative_path, file))
        .ok_or_else(|| format!("Postcondiția {operation} nu a găsit sursa finală {file}."))?;
    let before_untouched =
        contents_outside_line_envelopes(&before.contents, file, before_envelopes, operation)?;
    let after_untouched =
        contents_outside_line_envelopes(&after.contents, file, after_envelopes, operation)?;
    if before_untouched != after_untouched {
        return Err(format!(
            "Postcondiția {operation} a detectat octeți modificați în afara envelope-urilor structurale."
        ));
    }
    Ok(())
}

fn contents_outside_line_envelopes(
    source: &str,
    file: &str,
    nodes: &[&crate::source_graph::model::SourceNode],
    operation: &str,
) -> Result<Vec<u8>, String> {
    let mut spans = Vec::with_capacity(nodes.len());
    for node in nodes {
        if !same_model_file(&node.file, file) {
            return Err(format!(
                "Postcondiția {operation} a primit un envelope din alt fișier: {}.",
                node.file
            ));
        }
        let range = node.range.as_ref().ok_or_else(|| {
            format!(
                "Postcondiția {operation} nu are range pentru envelope-ul {}.",
                node.id
            )
        })?;
        if range.start > range.end || range.end > source.len() {
            return Err(format!(
                "Postcondiția {operation} are un envelope invalid pentru {}.",
                node.id
            ));
        }
        let line_start = source[..range.start]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let line_end = source[range.end..]
            .find('\n')
            .map(|relative| range.end + relative + 1)
            .unwrap_or(source.len());
        spans.push((line_start, line_end));
    }
    spans.sort_unstable();
    let mut merged = Vec::<(usize, usize)>::with_capacity(spans.len());
    for (start, end) in spans {
        if let Some((_, previous_end)) = merged.last_mut() {
            if start <= *previous_end {
                *previous_end = (*previous_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let mut untouched = Vec::with_capacity(source.len());
    let mut cursor = 0usize;
    for (start, end) in merged {
        untouched.extend_from_slice(&source.as_bytes()[cursor..start]);
        cursor = end;
    }
    untouched.extend_from_slice(&source.as_bytes()[cursor..]);
    Ok(untouched)
}

fn structural_kind_label(kind: &crate::source_graph::model::SourceNodeKind) -> &'static str {
    use crate::source_graph::model::SourceNodeKind;

    match kind {
        SourceNodeKind::Template => "template",
        SourceNodeKind::Partial => "partial",
        SourceNodeKind::Html => "html",
        SourceNodeKind::Extends => "extends",
        SourceNodeKind::Block => "block",
        SourceNodeKind::Include => "include",
        SourceNodeKind::Import => "import",
        SourceNodeKind::Macro => "macro",
        SourceNodeKind::For => "for",
        SourceNodeKind::If => "if",
        SourceNodeKind::Elif => "elif",
        SourceNodeKind::Else => "else",
        SourceNodeKind::Set => "set",
        SourceNodeKind::SetGlobal => "setGlobal",
        SourceNodeKind::Filter => "filter",
        SourceNodeKind::Break => "break",
        SourceNodeKind::Continue => "continue",
        SourceNodeKind::Super => "super",
        SourceNodeKind::TeraVariable => "teraVariable",
        SourceNodeKind::TeraComment => "teraComment",
        SourceNodeKind::Raw => "raw",
        SourceNodeKind::Tera => "tera",
        _ => "unsupported",
    }
}

fn validate_structural_relation(
    model: &ProjectModel,
    source: &crate::source_graph::model::SourceNode,
    target: &crate::source_graph::model::SourceNode,
    position: ProjectMovePosition,
    operation: &str,
) -> Result<(), String> {
    if source.id == target.id {
        return Err(format!(
            "Postcondiția {operation} a rezolvat aceeași ancoră pentru sursă și destinație."
        ));
    }
    let expected_parent = if position == ProjectMovePosition::Inside {
        Some(target.id.as_str())
    } else {
        target.parent.as_deref()
    };
    if source.parent.as_deref() != expected_parent {
        return Err(format!(
            "Postcondiția {operation} a găsit părintele {:?}, dar aștepta {:?}.",
            source.parent.as_deref(),
            expected_parent
        ));
    }
    let Some(parent_id) = expected_parent else {
        return Err(format!(
            "Postcondiția {operation} nu poate confirma ordinea fără un părinte structural."
        ));
    };
    let parent = model
        .source_graph
        .node_by_id(parent_id)
        .ok_or_else(|| format!("Postcondiția {operation} nu a găsit părintele {parent_id}."))?;
    let source_index = parent
        .children
        .iter()
        .position(|child| child == &source.id)
        .ok_or_else(|| format!("Postcondiția {operation} nu a găsit sursa în lista de copii."))?;
    match position {
        ProjectMovePosition::Inside if source_index + 1 == parent.children.len() => Ok(()),
        ProjectMovePosition::Before => {
            let target_index = parent
                .children
                .iter()
                .position(|child| child == &target.id)
                .ok_or_else(|| {
                    format!("Postcondiția {operation} nu a găsit destinația între frați.")
                })?;
            (source_index + 1 == target_index)
                .then_some(())
                .ok_or_else(|| format!("Postcondiția {operation} nu confirmă poziția before."))
        }
        ProjectMovePosition::After => {
            let target_index = parent
                .children
                .iter()
                .position(|child| child == &target.id)
                .ok_or_else(|| {
                    format!("Postcondiția {operation} nu a găsit destinația între frați.")
                })?;
            (target_index + 1 == source_index)
                .then_some(())
                .ok_or_else(|| format!("Postcondiția {operation} nu confirmă poziția after."))
        }
        ProjectMovePosition::Inside => Err(format!(
            "Postcondiția {operation} nu confirmă append-ul în interiorul destinației."
        )),
    }
}

fn validate_tera_relation(
    model: &ProjectModel,
    source: &crate::source_graph::model::SourceNode,
    target: &crate::source_graph::model::SourceNode,
    position: ProjectMovePosition,
    expected_child_index: Option<usize>,
    operation: &str,
) -> Result<(), String> {
    if position != ProjectMovePosition::Inside {
        return validate_structural_relation(model, source, target, position, operation);
    }
    if source.parent.as_deref() != Some(target.id.as_str()) {
        return Err(format!(
            "Postcondiția {operation} a găsit părintele {:?}, dar aștepta {}.",
            source.parent.as_deref(),
            target.id
        ));
    }
    let expected_child_index = expected_child_index
        .ok_or_else(|| format!("Postcondiția {operation} nu are indexul copilului așteptat."))?;
    let source_index = target
        .children
        .iter()
        .position(|child| child == &source.id)
        .ok_or_else(|| format!("Postcondiția {operation} nu a găsit copilul în destinație."))?;
    (source_index == expected_child_index)
        .then_some(())
        .ok_or_else(|| {
            format!(
                "Postcondiția {operation} a găsit indexul {source_index}, dar aștepta {expected_child_index}."
            )
        })
}

fn confirmed_html_relation(
    model: &ProjectModel,
    source: &crate::source_graph::model::SourceNode,
    target: &crate::source_graph::model::SourceNode,
    operation: &str,
) -> Result<ProjectMovePosition, String> {
    if source.parent.as_deref() == Some(target.id.as_str()) {
        let appended = target
            .children
            .last()
            .is_some_and(|child| child == &source.id);
        return appended
            .then_some(ProjectMovePosition::Inside)
            .ok_or_else(|| {
                format!("Relația confirmată pentru {operation} nu este append inside.")
            });
    }
    if source.parent != target.parent {
        return Err(format!(
            "Relația confirmată pentru {operation} are părinți diferiți."
        ));
    }
    let parent_id = source.parent.as_deref().ok_or_else(|| {
        format!("Relația confirmată pentru {operation} nu are părinte structural.")
    })?;
    let parent = model
        .source_graph
        .node_by_id(parent_id)
        .ok_or_else(|| format!("Relația confirmată nu a găsit părintele {parent_id}."))?;
    let source_index = parent
        .children
        .iter()
        .position(|child| child == &source.id)
        .ok_or_else(|| "Relația confirmată nu a găsit sursa între frați.".to_string())?;
    let target_index = parent
        .children
        .iter()
        .position(|child| child == &target.id)
        .ok_or_else(|| "Relația confirmată nu a găsit destinația între frați.".to_string())?;
    if source_index + 1 == target_index {
        Ok(ProjectMovePosition::Before)
    } else if target_index + 1 == source_index {
        Ok(ProjectMovePosition::After)
    } else {
        Err(format!(
            "Relația confirmată pentru {operation} nu este adiacentă."
        ))
    }
}

fn html_label_has_tag(label: &str, tag: &str) -> bool {
    let label = label.trim_start();
    let tag = tag.trim().to_ascii_lowercase();
    label.strip_prefix('<').is_some_and(|rest| {
        rest.get(..tag.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&tag))
            && rest
                .as_bytes()
                .get(tag.len())
                .is_none_or(|byte| matches!(byte, b' ' | b'>' | b'.' | b'#'))
    })
}

fn same_model_file(left: &str, right: &str) -> bool {
    left.trim_start_matches('/').replace('\\', "/")
        == right.trim_start_matches('/').replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::{
        duplicate_engine::{plan_html_duplicate, ProjectHtmlDuplicateIntent},
        insert_engine::{plan_html_insert, ProjectHtmlInsertElement, ProjectHtmlInsertIntent},
        move_engine::{plan_html_move, ProjectHtmlMoveIntent},
        rebuild_project_model_after_workspace_change_with_source_changes,
        tera_insert_engine::{
            plan_tera_insert_for_active_document, ProjectTeraInsertIntent, ProjectTeraInsertItem,
        },
        tera_move_engine::{plan_tera_move, ProjectTeraMoveIntent},
        test_support::ProjectModelTestFixture,
    };

    use super::super::spec::{EDITOR_MOVE_PLAN, HTML_INSERT_DROP_PLAN};
    use super::*;

    fn build_reconciled_after<P: PreviewStructuralPatch>(
        fixture: &mut ProjectModelTestFixture,
        before: &ProjectModel,
        patch: &P,
    ) -> ProjectModel {
        let previous_revision = fixture.projection().revision;
        let result_revision = previous_revision + 1;
        let source_changes = patch.source_changes(before).unwrap();
        fixture.draft(patch.file(), patch.contents());
        fixture.revision(
            result_revision,
            Some(format!("postcondition-{result_revision}")),
        );
        let projection = fixture.projection();
        rebuild_project_model_after_workspace_change_with_source_changes(
            fixture.root(),
            Some(before),
            Some(previous_revision),
            &projection,
            &[patch.file().to_string()],
            patch.project_model_incremental_intent(),
            source_changes,
        )
        .unwrap()
        .model
    }

    #[test]
    fn structural_plan_patch_or_block_uses_plan_diagnostic_when_missing_patch() {
        let plan = ProjectHtmlMovePlan {
            allowed: false,
            diagnostic: Some("Ancora nu mai există.".to_string()),
            model_revision: "model-1".to_string(),
            patch: None,
        };

        let blocked =
            structural_plan_patch_or_block::<ProjectHtmlMovePatch, _>(plan, EDITOR_MOVE_PLAN)
                .expect_err("plan fără patch trebuie blocat");

        assert_eq!(blocked.model_revision, "model-1");
        assert_eq!(blocked.diagnostic.code, "editor_move_plan_became_stale");
        assert_eq!(
            blocked.diagnostic.diagnostic.code,
            "preview-projection-structural-plan-blocked-with-details"
        );
        assert_eq!(
            blocked.diagnostic.diagnostic.arguments.get("details"),
            Some(&serde_json::Value::String(
                "Ancora nu mai există.".to_string()
            ))
        );
        assert!(blocked.diagnostic.blocking);
    }

    #[test]
    fn structural_plan_patch_or_block_stays_semantic_without_engine_diagnostic() {
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
        assert!(blocked.diagnostic.diagnostic.arguments.is_empty());
        assert!(blocked.diagnostic.blocking);
    }

    #[test]
    fn html_move_postcondition_confirms_parent_and_order_and_rejects_drift() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pana-postcondition-{stamp}"));
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "<main>\n",
                "  <div class=\"target\">\n",
                "    <p>A</p>\n",
                "  </div>\n",
                "  <article class=\"source\">B</article>\n",
                "</main>\n",
            ),
        )
        .unwrap();
        let before = fixture.build_model().unwrap();
        let source_id = before
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<article .source>")
            .unwrap()
            .id
            .clone();
        let target_id = before
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<div .target>")
            .unwrap()
            .id
            .clone();
        let patch = plan_html_move(
            &before,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(source_id),
                target_source_id: Some(target_id),
                source_tag: Some("article".to_string()),
                target_tag: Some("div".to_string()),
                position: ProjectMovePosition::Inside,
                native_block_slot: None,
            },
        )
        .patch
        .expect("move patch");
        let after = build_reconciled_after(&mut fixture, &before, &patch);
        validate_html_move_after_model(&before, &after, &patch).unwrap();

        let mut drifted = after.clone();
        let moved = drifted
            .source_graph
            .nodes
            .iter_mut()
            .find(|node| {
                node.label == patch.source_label
                    && node
                        .range
                        .as_ref()
                        .is_some_and(|range| range.line == patch.new_start_line)
            })
            .unwrap();
        moved.parent = None;
        assert!(validate_html_move_after_model(&before, &drifted, &patch).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn html_insert_and_duplicate_postconditions_confirm_exact_relations() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pana-postcondition-matrix-{stamp}"));
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "<main>\n",
                "  <div class=\"target\"></div>\n",
                "  <article class=\"card\">Text</article>\n",
                "</main>\n",
            ),
        )
        .unwrap();
        let before_insert = fixture.build_model().unwrap();
        let target_id = before_insert
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<div .target>")
            .unwrap()
            .id
            .clone();
        let insert_patch = plan_html_insert(
            &before_insert,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(target_id),
                target_tag: Some("div".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "p".to_string(),
                    class_name: Some("inserted".to_string()),
                    text: Some("Nou".to_string()),
                    label: Some("Paragraf".to_string()),
                },
                native_block_slot: None,
            },
            Some("templates/index.html"),
        )
        .patch
        .expect("insert patch");
        let after_insert = build_reconciled_after(&mut fixture, &before_insert, &insert_patch);
        validate_html_insert_after_model(&before_insert, &after_insert, &insert_patch).unwrap();

        let card_id = after_insert
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<article .card>")
            .unwrap()
            .id
            .clone();
        let duplicate_patch = plan_html_duplicate(
            &after_insert,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(card_id),
                source_tag: Some("article".to_string()),
                native_block_slot: None,
            },
        )
        .patch
        .expect("duplicate patch");
        let after_duplicate = build_reconciled_after(&mut fixture, &after_insert, &duplicate_patch);
        validate_html_duplicate_after_model(&after_insert, &after_duplicate, &duplicate_patch)
            .unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn html_insert_before_identical_sibling_cannot_steal_existing_source_node_id() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pana-identical-insert-{stamp}"));
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            "<main>\n  <p>Același</p>\n</main>\n",
        )
        .unwrap();
        let before = fixture.build_model().unwrap();
        let existing = before
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<p>")
            .unwrap();
        let existing_id = existing.id.clone();
        let main_id = existing.parent.clone().unwrap();
        let patch = plan_html_insert(
            &before,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(existing_id.clone()),
                target_tag: Some("p".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Before,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "p".to_string(),
                    class_name: None,
                    text: Some("Același".to_string()),
                    label: Some("Paragraf identic".to_string()),
                },
                native_block_slot: None,
            },
            Some("templates/index.html"),
        )
        .patch
        .expect("identical insert patch");
        let after = build_reconciled_after(&mut fixture, &before, &patch);
        validate_html_insert_after_model(&before, &after, &patch).unwrap();

        let main = after.source_graph.node_by_id(&main_id).unwrap();
        let paragraphs = main
            .children
            .iter()
            .filter(|id| {
                after
                    .source_graph
                    .node_by_id(id)
                    .is_some_and(|node| node.label == "<p>")
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(paragraphs.len(), 2);
        assert_ne!(paragraphs[0], existing_id);
        assert_eq!(paragraphs[1], existing_id);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tera_insert_and_move_postconditions_confirm_exact_parent_and_order() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pana-tera-postcondition-{stamp}"));
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  {{ first }}\n",
                "  <section class=\"target\"></section>\n",
                "  {{ last }}\n",
                "{% endblock content %}\n",
            ),
        )
        .unwrap();
        let before_move = fixture.build_model().unwrap();
        let source = before_move
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::TeraVariable
                    && node.label == "last"
            })
            .unwrap();
        let target = before_move
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .target>")
            .unwrap();
        let move_patch = plan_tera_move(
            &before_move,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_kind: Some("teraVariable".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Before,
            },
        )
        .patch
        .expect("tera move patch");
        let after_move = build_reconciled_after(&mut fixture, &before_move, &move_patch);
        validate_structural_projection(
            &before_move,
            &after_move,
            &move_patch.file,
            &move_patch.contents,
        )
        .unwrap();
        validate_tera_move_after_model(&before_move, &after_move, &move_patch).unwrap();

        let block = after_move
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::Block
                    && node.label == "content"
            })
            .unwrap();
        let insert_patch = plan_tera_insert_for_active_document(
            &after_move,
            &ProjectTeraInsertIntent {
                target_source_id: Some(block.id.clone()),
                target_kind: Some("block".to_string()),
                target_tag: None,
                position: ProjectMovePosition::Inside,
                item: ProjectTeraInsertItem {
                    kind: "teraVariable".to_string(),
                    label: Some("Titlu".to_string()),
                    target: None,
                    name: None,
                    expression: Some("page.title".to_string()),
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        )
        .patch
        .expect("tera insert patch");
        let after_insert = build_reconciled_after(&mut fixture, &after_move, &insert_patch);
        validate_structural_projection(
            &after_move,
            &after_insert,
            &insert_patch.file,
            &insert_patch.contents,
        )
        .unwrap();
        validate_tera_insert_after_model(&after_move, &after_insert, &insert_patch).unwrap();

        let mut drifted = after_insert.clone();
        let inserted = drifted
            .source_graph
            .nodes
            .iter_mut()
            .find(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::TeraVariable
                    && node.label == "page.title"
            })
            .unwrap();
        inserted.parent = None;
        assert!(validate_tera_insert_after_model(&after_move, &drifted, &insert_patch).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tera_macro_call_insert_projects_and_retains_both_new_roots() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pana-tera-forest-{stamp}"));
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<main></main>\n").unwrap();
        fixture.source(
            "templates/macros.html",
            "{% macro card() %}<article></article>{% endmacro %}\n",
        );
        let before = fixture.build_model().unwrap();
        let target = before
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();
        let patch = plan_tera_insert_for_active_document(
            &before,
            &ProjectTeraInsertIntent {
                target_source_id: Some(target.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "macroCall".to_string(),
                    label: Some("Card".to_string()),
                    target: Some("macros.html".to_string()),
                    name: Some("card".to_string()),
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        )
        .patch
        .expect("macro call insert patch");
        let after = build_reconciled_after(&mut fixture, &before, &patch);
        validate_tera_insert_after_model(&before, &after, &patch).unwrap();
        let inserted = inserted_tera_source_nodes(&before, &after, &patch).unwrap();
        assert_eq!(inserted.len(), 2);
        let inserted_ids = inserted
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let forest = crate::source_graph::identity::capture_source_forest_identity(
            &after.source_graph,
            &inserted_ids,
        )
        .unwrap();
        assert_eq!(forest.root_count, 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tera_inside_html_uses_the_confirmed_html_parent_and_append_index() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pana-tera-html-parent-{stamp}"));
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  <section class=\"target\">\n",
                "    <span>Copil</span>\n",
                "  </section>\n",
                "  {{ moving }}\n",
                "{% endblock content %}\n",
            ),
        )
        .unwrap();
        let before_move = fixture.build_model().unwrap();
        let source = before_move
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::TeraVariable
                    && node.label == "moving"
            })
            .unwrap();
        let target = before_move
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .target>")
            .unwrap();
        let move_patch = plan_tera_move(
            &before_move,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_kind: Some("teraVariable".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Inside,
            },
        )
        .patch
        .expect("tera inside html move patch");
        assert_eq!(
            move_patch.contents,
            concat!(
                "{% block content %}\n",
                "  <section class=\"target\">\n",
                "    <span>Copil</span>\n",
                "    {{ moving }}\n",
                "  </section>\n",
                "{% endblock content %}\n",
            )
        );
        let after_move = build_reconciled_after(&mut fixture, &before_move, &move_patch);
        validate_tera_move_after_model(&before_move, &after_move, &move_patch).unwrap();

        let target = after_move
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .target>")
            .unwrap();
        let insert_patch = plan_tera_insert_for_active_document(
            &after_move,
            &ProjectTeraInsertIntent {
                target_source_id: Some(target.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Inside,
                item: ProjectTeraInsertItem {
                    kind: "teraVariable".to_string(),
                    label: Some("Titlu".to_string()),
                    target: None,
                    name: None,
                    expression: Some("page.title".to_string()),
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        )
        .patch
        .expect("tera inside html insert patch");
        assert_eq!(
            insert_patch.contents,
            concat!(
                "{% block content %}\n",
                "  <section class=\"target\">\n",
                "    <span>Copil</span>\n",
                "    {{ moving }}\n",
                "    {{ page.title }}\n",
                "  </section>\n",
                "{% endblock content %}\n",
            )
        );
        let after_insert = build_reconciled_after(&mut fixture, &after_move, &insert_patch);
        validate_tera_insert_after_model(&after_move, &after_insert, &insert_patch).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

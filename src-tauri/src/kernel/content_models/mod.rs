mod catalog;
mod mutation_plan;
mod rewrite;
mod staging;
mod usage_index;
mod validation;

pub use super::content_schema::{
    ContentFieldChoice, ContentFieldDefinition, ContentFieldKind, ContentModelDefinition,
    CustomFieldTemplateUsage,
};
pub(crate) use catalog::build_content_model_catalog_from_workspace_projection;
pub use catalog::{
    ContentModelAssignment, ContentModelCatalog, ContentModelDiagnostic, ContentModelPageBinding,
};
pub use mutation_plan::{
    plan_content_model_mutation, ContentModelMutationInput, ContentModelMutationOperation,
    ContentModelMutationPlan, PlannedContentModelMutation,
};
pub use staging::stage_content_model_mutation;
pub use usage_index::refresh_content_model_template_usages;
pub(crate) use usage_index::upsert_content_model_template_usages;
pub use validation::validate_model;

pub const CONTENT_MODEL_SCHEMA_VERSION: u32 = 1;
pub const CONTENT_MODEL_PROJECT_PATH: &str = ".panastudio/project.toml";
pub const CONTENT_MODEL_ASSIGNMENTS_PATH: &str = ".panastudio/assignments.toml";
const CONTENT_MODEL_DIRECTORY: &str = ".panastudio/content-models";

#[cfg(test)]
use rewrite::{frontmatter::*, templates::*};
#[cfg(test)]
use usage_index::expression_offsets;
#[cfg(test)]
use validation::*;
#[cfg(test)]
mod tests;

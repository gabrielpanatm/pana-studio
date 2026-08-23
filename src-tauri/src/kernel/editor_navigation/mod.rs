use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    blocks::NativeBlockSlotMutationContext,
    kernel::{
        dynamic_widgets::DynamicWidgetProperties, preview_projection::CanvasPatch,
        project_workspace::ProjectWorkspaceMutationReceipt,
    },
    preview::{
        CanvasBoundaryInstance, CanvasGraph, CanvasMarkdownProvenanceState, CanvasNodeOrigin,
        CanvasProjectionIdentity, CanvasRenderNode,
    },
    project_model::{
        attribute_engine::raw_tag_attributes,
        model::ProjectModel,
        move_engine::{
            parse_html_tag_at, plan_html_move, plan_html_move_in_edit_scope, ProjectHtmlMoveIntent,
            ProjectMovePosition,
        },
        tera_move_engine::{plan_tera_move, ProjectTeraMoveIntent},
    },
    source_graph::model::{
        ComponentDefinitionKind, ComponentInvocation, ComponentInvocationKind,
        ComponentResolutionStatus, SourceCapabilityReason, SourceGraphTemplate, SourceNode,
        SourceNodeKind, SourceOrigin, SourceRange, SourceRelationKind,
    },
};

mod access;
mod contracts;
mod move_planner;
mod provenance;
mod runtime;
mod snapshot;
mod view;

pub(crate) use access::editor_navigation_access_node;
pub use contracts::*;
pub(crate) use move_planner::{
    editor_navigation_node, plan_editor_move, plan_editor_move_with_slot,
};
pub use runtime::EditorNavigationRuntime;
pub(crate) use snapshot::build_editor_navigation_snapshot;

#[cfg(test)]
pub(crate) use runtime::editor_navigation_snapshot_for_test;
#[cfg(test)]
use {move_planner::*, provenance::*, snapshot::*, view::*};
#[cfg(test)]
mod tests;

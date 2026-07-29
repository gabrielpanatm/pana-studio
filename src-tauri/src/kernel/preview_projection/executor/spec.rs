use super::super::model::PreviewProjectionIntentKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreviewExecutorIntentSpec {
    pub(super) expected_kind: PreviewProjectionIntentKind,
    pub(super) wrong_kind_code: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreviewStructuralPlanSpec {
    pub(super) write_label: &'static str,
    pub(super) blocked_code: &'static str,
}

pub(super) const EDITOR_MOVE_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Editor semantic move",
    blocked_code: "editor_move_plan_became_stale",
};

pub(super) const HTML_INSERT_DROP_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlInsertDrop,
    wrong_kind_code: "preview_html_insert_drop_wrong_intent_kind",
};

pub(super) const HTML_INSERT_DROP_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML insert drop",
    blocked_code: "preview_html_insert_drop_plan_blocked",
};

pub(super) const HTML_ATTRIBUTES_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlAttributes,
    wrong_kind_code: "preview_html_attributes_wrong_intent_kind",
};

pub(super) const HTML_ATTRIBUTES_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML attributes",
    blocked_code: "preview_html_attributes_plan_blocked",
};

pub(super) const HTML_TEXT_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlText,
    wrong_kind_code: "preview_html_text_wrong_intent_kind",
};

pub(super) const HTML_TEXT_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML text",
    blocked_code: "preview_html_text_plan_blocked",
};

pub(super) const HTML_TAG_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlTag,
    wrong_kind_code: "preview_html_tag_wrong_intent_kind",
};

pub(super) const HTML_TAG_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML tag",
    blocked_code: "preview_html_tag_plan_blocked",
};

pub(super) const HTML_DUPLICATE_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlDuplicate,
    wrong_kind_code: "preview_html_duplicate_wrong_intent_kind",
};

pub(super) const HTML_DUPLICATE_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML duplicate",
    blocked_code: "preview_html_duplicate_plan_blocked",
};

pub(super) const HTML_DELETE_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlDelete,
    wrong_kind_code: "preview_html_delete_wrong_intent_kind",
};

pub(super) const HTML_DELETE_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML delete",
    blocked_code: "preview_html_delete_plan_blocked",
};

pub(super) const TERA_INSERT_DROP_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::TeraInsertDrop,
    wrong_kind_code: "preview_tera_insert_drop_wrong_intent_kind",
};

pub(super) const TERA_INSERT_DROP_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview Tera insert drop",
    blocked_code: "preview_tera_insert_drop_plan_blocked",
};

pub(super) const TERA_DELETE_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::TemplateDelete,
    wrong_kind_code: "preview_tera_delete_wrong_intent_kind",
};

pub(super) const TERA_DELETE_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview Tera delete",
    blocked_code: "preview_tera_delete_plan_blocked",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_executor_intent_specs_keep_expected_kinds_distinct() {
        let specs = [
            HTML_INSERT_DROP_INTENT,
            HTML_ATTRIBUTES_INTENT,
            HTML_TEXT_INTENT,
            HTML_TAG_INTENT,
            HTML_DUPLICATE_INTENT,
            HTML_DELETE_INTENT,
            TERA_INSERT_DROP_INTENT,
            TERA_DELETE_INTENT,
        ];

        for spec in specs {
            assert_ne!(spec.expected_kind, PreviewProjectionIntentKind::Unsupported);
            assert!(!spec.wrong_kind_code.trim().is_empty());
        }
    }

    #[test]
    fn preview_structural_plan_specs_have_commit_and_blocking_contracts() {
        let specs = [
            EDITOR_MOVE_PLAN,
            HTML_INSERT_DROP_PLAN,
            HTML_ATTRIBUTES_PLAN,
            HTML_TEXT_PLAN,
            HTML_TAG_PLAN,
            HTML_DUPLICATE_PLAN,
            HTML_DELETE_PLAN,
            TERA_INSERT_DROP_PLAN,
            TERA_DELETE_PLAN,
        ];

        for spec in specs {
            assert!(!spec.write_label.trim().is_empty());
            assert!(!spec.blocked_code.trim().is_empty());
        }
    }
}

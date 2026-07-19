use super::super::model::PreviewProjectionIntentKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreviewExecutorIntentSpec {
    pub(super) expected_kind: PreviewProjectionIntentKind,
    pub(super) wrong_kind_code: &'static str,
    pub(super) wrong_kind_message: &'static str,
    pub(super) preflight_blocked_message: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreviewStructuralPlanSpec {
    pub(super) write_label: &'static str,
    pub(super) blocked_code: &'static str,
    pub(super) blocked_fallback: &'static str,
    pub(super) blocked_message: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreviewTemplatePermissionPlanSpec {
    pub(super) blocked_code: &'static str,
    pub(super) blocked_fallback: &'static str,
    pub(super) blocked_message: &'static str,
}

pub(super) const LAYER_DROP_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::LayerDrop,
    wrong_kind_code: "preview_layer_drop_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-layer-drop refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview layer drop blocat înainte de execuție.",
};

pub(super) const LAYER_DROP_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview layer drop",
    blocked_code: "preview_layer_drop_move_plan_blocked",
    blocked_fallback: "Move Engine-ul a blocat mutarea fără diagnostic specific.",
    blocked_message: "Preview layer drop blocat de Move Engine.",
};

pub(super) const HTML_INSERT_DROP_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlInsertDrop,
    wrong_kind_code: "preview_html_insert_drop_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-insert-drop refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview HTML insert drop blocat înainte de execuție.",
};

pub(super) const HTML_INSERT_DROP_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML insert drop",
    blocked_code: "preview_html_insert_drop_plan_blocked",
    blocked_fallback: "HTML Insert Engine-ul a blocat inserarea fără diagnostic specific.",
    blocked_message: "Preview HTML insert drop blocat de Insert Engine.",
};

pub(super) const HTML_ATTRIBUTES_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlAttributes,
    wrong_kind_code: "preview_html_attributes_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-html-attributes refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview HTML attributes blocat înainte de execuție.",
};

pub(super) const HTML_ATTRIBUTES_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML attributes",
    blocked_code: "preview_html_attributes_plan_blocked",
    blocked_fallback: "HTML Attribute Engine-ul a blocat atributele fără diagnostic specific.",
    blocked_message: "Preview HTML attributes blocat de Attribute Engine.",
};

pub(super) const HTML_TEXT_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlText,
    wrong_kind_code: "preview_html_text_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-html-text refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview HTML text blocat înainte de execuție.",
};

pub(super) const HTML_TEXT_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML text",
    blocked_code: "preview_html_text_plan_blocked",
    blocked_fallback: "HTML Text Engine-ul a blocat textul fără diagnostic specific.",
    blocked_message: "Preview HTML text blocat de Text Engine.",
};

pub(super) const HTML_TAG_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlTag,
    wrong_kind_code: "preview_html_tag_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-html-tag refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview HTML tag blocat înainte de execuție.",
};

pub(super) const HTML_TAG_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML tag",
    blocked_code: "preview_html_tag_plan_blocked",
    blocked_fallback: "HTML Tag Engine-ul a blocat schimbarea fără diagnostic specific.",
    blocked_message: "Preview HTML tag blocat de Tag Engine.",
};

pub(super) const HTML_DUPLICATE_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlDuplicate,
    wrong_kind_code: "preview_html_duplicate_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-duplicate-selected refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview HTML duplicate blocat înainte de execuție.",
};

pub(super) const HTML_DUPLICATE_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML duplicate",
    blocked_code: "preview_html_duplicate_plan_blocked",
    blocked_fallback: "HTML Duplicate Engine-ul a blocat duplicarea fără diagnostic specific.",
    blocked_message: "Preview HTML duplicate blocat de Duplicate Engine.",
};

pub(super) const HTML_DELETE_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::HtmlDelete,
    wrong_kind_code: "preview_html_delete_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-delete-selected refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview HTML delete blocat înainte de execuție.",
};

pub(super) const HTML_DELETE_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview HTML delete",
    blocked_code: "preview_html_delete_plan_blocked",
    blocked_fallback: "HTML Delete Engine-ul a blocat ștergerea fără diagnostic specific.",
    blocked_message: "Preview HTML delete blocat de Delete Engine.",
};

pub(super) const TERA_INSERT_DROP_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::TeraInsertDrop,
    wrong_kind_code: "preview_tera_insert_drop_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-tera-drop refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview Tera insert drop blocat înainte de execuție.",
};

pub(super) const TERA_INSERT_DROP_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview Tera insert drop",
    blocked_code: "preview_tera_insert_drop_plan_blocked",
    blocked_fallback: "Tera Insert Engine-ul a blocat inserarea fără diagnostic specific.",
    blocked_message: "Preview Tera insert drop blocat de Insert Engine.",
};

pub(super) const TERA_MOVE_DROP_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::TeraMoveDrop,
    wrong_kind_code: "preview_tera_move_drop_wrong_intent_kind",
    wrong_kind_message: "Executorul preview-tera-move-drop refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview Tera move drop blocat înainte de execuție.",
};

pub(super) const TERA_MOVE_DROP_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview Tera move drop",
    blocked_code: "preview_tera_move_drop_plan_blocked",
    blocked_fallback: "Tera Move Engine-ul a blocat mutarea fără diagnostic specific.",
    blocked_message: "Preview Tera move drop blocat de Move Engine.",
};

pub(super) const TERA_DELETE_INTENT: PreviewExecutorIntentSpec = PreviewExecutorIntentSpec {
    expected_kind: PreviewProjectionIntentKind::TemplateDelete,
    wrong_kind_code: "preview_tera_delete_wrong_intent_kind",
    wrong_kind_message:
        "Executorul preview-template-delete-selected refuză orice alt tip de intenție.",
    preflight_blocked_message: "Preview Tera delete blocat înainte de execuție.",
};

pub(super) const TERA_DELETE_PLAN: PreviewStructuralPlanSpec = PreviewStructuralPlanSpec {
    write_label: "Preview Tera delete",
    blocked_code: "preview_tera_delete_plan_blocked",
    blocked_fallback: "Tera Delete Engine-ul a blocat ștergerea fără diagnostic specific.",
    blocked_message: "Preview Tera delete blocat de Delete Engine.",
};

pub(super) const TEMPLATE_EDIT_PERMISSION_INTENT: PreviewExecutorIntentSpec =
    PreviewExecutorIntentSpec {
        expected_kind: PreviewProjectionIntentKind::TemplateEdit,
        wrong_kind_code: "preview_template_edit_wrong_intent_kind",
        wrong_kind_message:
            "Executorul preview-template-edit-selected refuză orice alt tip de intenție.",
        preflight_blocked_message: "Preview template edit permission blocat înainte de execuție.",
    };

pub(super) const TEMPLATE_EDIT_PERMISSION_PLAN: PreviewTemplatePermissionPlanSpec =
    PreviewTemplatePermissionPlanSpec {
        blocked_code: "preview_template_edit_permission_blocked",
        blocked_fallback: "Template Edit Gate-ul a blocat deblocarea fără diagnostic specific.",
        blocked_message: "Preview template edit permission blocat de Project Model.",
    };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_executor_intent_specs_keep_expected_kinds_distinct() {
        let specs = [
            LAYER_DROP_INTENT,
            HTML_INSERT_DROP_INTENT,
            HTML_ATTRIBUTES_INTENT,
            HTML_TEXT_INTENT,
            HTML_TAG_INTENT,
            HTML_DUPLICATE_INTENT,
            HTML_DELETE_INTENT,
            TERA_INSERT_DROP_INTENT,
            TERA_MOVE_DROP_INTENT,
            TERA_DELETE_INTENT,
            TEMPLATE_EDIT_PERMISSION_INTENT,
        ];

        for spec in specs {
            assert_ne!(spec.expected_kind, PreviewProjectionIntentKind::Unsupported);
            assert!(!spec.wrong_kind_code.trim().is_empty());
            assert!(!spec.wrong_kind_message.trim().is_empty());
            assert!(!spec.preflight_blocked_message.trim().is_empty());
        }
    }

    #[test]
    fn preview_structural_plan_specs_have_commit_and_blocking_contracts() {
        let specs = [
            LAYER_DROP_PLAN,
            HTML_INSERT_DROP_PLAN,
            HTML_ATTRIBUTES_PLAN,
            HTML_TEXT_PLAN,
            HTML_TAG_PLAN,
            HTML_DUPLICATE_PLAN,
            HTML_DELETE_PLAN,
            TERA_INSERT_DROP_PLAN,
            TERA_MOVE_DROP_PLAN,
            TERA_DELETE_PLAN,
        ];

        for spec in specs {
            assert!(!spec.write_label.trim().is_empty());
            assert!(!spec.blocked_code.trim().is_empty());
            assert!(!spec.blocked_fallback.trim().is_empty());
            assert!(!spec.blocked_message.trim().is_empty());
        }
    }
}

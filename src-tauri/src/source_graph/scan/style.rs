use std::path::Path;

use crate::source_graph::{
    model::{SourceCapabilities, SourceNodeKind, SourceOrigin, SourceStyleScope},
    scan::{
        builder::SourceGraphBuilder,
        files::relative_project_path,
        summary::{StyleSummary, TemplateSummary},
    },
};
use crate::zola_theme::{zola_path_without_theme_root, ZolaTemplateOrigin, ZolaThemeResolver};

pub(super) fn scan_style(
    project_root: &Path,
    path: &Path,
    origin: SourceOrigin,
    theme_name: Option<String>,
    builder: &mut SourceGraphBuilder,
) -> StyleSummary {
    let file = relative_project_path(project_root, path);
    let node_id = builder.add_node(
        SourceNodeKind::Style,
        file.clone(),
        origin.clone(),
        theme_name.clone(),
        file.clone(),
        None,
        None,
        SourceCapabilities::code_only("Fișier de stil."),
    );
    StyleSummary {
        file,
        node_id,
        origin,
        theme_name,
    }
}

pub(super) fn conventional_style_files_for_template(
    resolver: &ZolaThemeResolver,
    template: &TemplateSummary,
) -> Vec<String> {
    resolver.conventional_style_files_for_template(&template.name, &template_origin(template), true)
}

fn template_origin(template: &TemplateSummary) -> ZolaTemplateOrigin {
    match (&template.origin, template.theme_name.as_ref()) {
        (SourceOrigin::Theme, Some(theme)) => ZolaTemplateOrigin::Theme(theme.clone()),
        _ => ZolaTemplateOrigin::Local,
    }
}

pub(super) fn style_scope_for_file(file: &str) -> SourceStyleScope {
    let theme_relative = zola_path_without_theme_root(file);
    if theme_relative.starts_with("sass/css-framework/")
        || theme_relative == "sass/framework.scss"
        || theme_relative.starts_with("static/css/")
    {
        SourceStyleScope::Global
    } else if theme_relative.starts_with("sass/pagini/") {
        SourceStyleScope::Page
    } else if theme_relative.starts_with("sass/partials/")
        || theme_relative.starts_with("sass/componente/")
    {
        SourceStyleScope::Partial
    } else {
        SourceStyleScope::Other
    }
}

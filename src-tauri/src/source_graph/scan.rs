use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

mod asset;
mod builder;
mod data_file;
mod files;
mod page;
mod ranges;
mod relations;
mod style;
mod summary;
mod template;

use crate::{
    kernel::project_workspace::WorkspaceProjectionLease,
    project::{is_zola_project, zola_project_root},
    source_graph::{
        model::{
            SourceDiagnosticSeverity, SourceGraph, SourceGraphAsset, SourceGraphDataFile,
            SourceGraphScript, SourceGraphStyle, SourceGraphTemplate,
        },
        scan::{
            asset::scan_asset,
            builder::SourceGraphBuilder,
            data_file::{scan_data_file, ZOLA_DATA_FILE_EXTENSIONS},
            files::{
                apply_virtual_file_projection, collect_all_files, collect_files_with_extension,
                collect_files_with_extensions, require_safe_deleted_source_paths,
                require_safe_draft_source_paths, require_safe_scan_root,
            },
            page::scan_content_page,
            relations::{
                add_template_asset_relations, add_template_content_relations,
                add_template_load_data_relations, add_template_relations,
                add_template_style_relations, asset_reference_map, block_node_map,
                content_node_map, data_file_reference_map, template_node_map, template_summary_map,
            },
            style::{scan_style, style_scope_for_file},
            template::scan_template,
        },
    },
    zola_theme::{active_theme_from_source, ZolaThemeResolver},
};

pub fn build_source_graph(project_root: &Path) -> Result<SourceGraph, String> {
    build_source_graph_with_drafts(project_root, &HashMap::new())
}

pub fn build_source_graph_with_drafts(
    project_root: &Path,
    draft_sources: &HashMap<String, String>,
) -> Result<SourceGraph, String> {
    build_source_graph_with_projection(project_root, draft_sources, &HashSet::new())
}

pub fn build_source_graph_with_projection(
    project_root: &Path,
    draft_sources: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
) -> Result<SourceGraph, String> {
    build_source_graph_internal(project_root, draft_sources, deleted_sources, None)
}

pub fn build_source_graph_from_workspace_projection(
    project_root: &Path,
    projection: &WorkspaceProjectionLease,
) -> Result<SourceGraph, String> {
    build_source_graph_internal(
        project_root,
        &projection.source_texts,
        &projection.deleted_sources,
        Some(projection),
    )
}

fn build_source_graph_internal(
    project_root: &Path,
    draft_sources: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
    workspace_projection: Option<&WorkspaceProjectionLease>,
) -> Result<SourceGraph, String> {
    let root = project_root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul proiectului: {}", error))?;
    if let Some(projection) = workspace_projection {
        if root != Path::new(&projection.project_root) {
            return Err(format!(
                "Source Graph a refuzat proiecția pentru alt root: {} != {}.",
                root.display(),
                projection.project_root
            ));
        }
    }
    let zola_root = zola_project_root(&root);
    let _ = require_safe_scan_root(&zola_root)?;
    require_safe_draft_source_paths(draft_sources)?;
    require_safe_deleted_source_paths(deleted_sources)?;
    let projected_config = ["sursa/zola.toml", "sursa/config.toml"]
        .iter()
        .find_map(|path| draft_sources.get(*path));
    let theme_resolver = match workspace_projection {
        Some(_) => ZolaThemeResolver::new(
            projected_config.and_then(|source| active_theme_from_source(source)),
        ),
        None => ZolaThemeResolver::for_root(&zola_root),
    };
    let active_theme = theme_resolver.active_theme().map(str::to_string);
    let mut builder = SourceGraphBuilder::new(&root, &zola_root, active_theme.clone());

    let is_zola = match workspace_projection {
        Some(_) => projected_config.is_some(),
        None => is_zola_project(&root),
    };
    if !is_zola {
        builder.add_diagnostic(
            SourceDiagnosticSeverity::Warning,
            "Proiectul curent nu pare să fie un proiect Zola valid.",
            None,
            None,
        );
        return Ok(builder.finish(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));
    }

    let mut content_files = if workspace_projection.is_some() {
        Vec::new()
    } else {
        collect_files_with_extension(&zola_root.join("content"), "md")?
    };
    let mut template_files = if workspace_projection.is_some() {
        Vec::new()
    } else {
        collect_files_with_extension(&zola_root.join("templates"), "html")?
    };
    let mut style_files = if workspace_projection.is_some() {
        Vec::new()
    } else {
        let mut files = collect_files_with_extension(&zola_root.join("sass"), "scss")?;
        files.extend(collect_files_with_extensions(
            &zola_root.join("static"),
            &["css", "scss"],
        )?);
        files
    };
    let mut asset_files = if workspace_projection.is_some() {
        Vec::new()
    } else {
        collect_all_files(&zola_root.join("static"))?
    };
    let mut data_file_paths = if workspace_projection.is_some() {
        Vec::new()
    } else {
        collect_files_with_extensions(&zola_root.join("date"), ZOLA_DATA_FILE_EXTENSIONS)?
    };

    let mut theme_template_files = Vec::new();
    let mut theme_style_files = Vec::new();
    let mut theme_asset_files = Vec::new();
    if let Some(theme) = active_theme.as_ref() {
        let theme_root = zola_root.join("themes").join(theme);
        if workspace_projection.is_none() {
            theme_template_files =
                collect_files_with_extension(&theme_root.join("templates"), "html")?;
            theme_style_files = collect_files_with_extension(&theme_root.join("sass"), "scss")?;
            theme_style_files.extend(collect_files_with_extensions(
                &theme_root.join("static"),
                &["css", "scss"],
            )?);
            theme_asset_files = collect_all_files(&theme_root.join("static"))?;
        }
    }
    apply_virtual_file_projection(
        &root,
        &zola_root.join("content"),
        Some(&["md"]),
        draft_sources,
        deleted_sources,
        &mut content_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("templates"),
        Some(&["html"]),
        draft_sources,
        deleted_sources,
        &mut template_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("sass"),
        Some(&["scss"]),
        draft_sources,
        deleted_sources,
        &mut style_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("static"),
        Some(&["css", "scss"]),
        draft_sources,
        deleted_sources,
        &mut style_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("static"),
        None,
        draft_sources,
        deleted_sources,
        &mut asset_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("date"),
        Some(ZOLA_DATA_FILE_EXTENSIONS),
        draft_sources,
        deleted_sources,
        &mut data_file_paths,
    )?;
    if let Some(theme) = active_theme.as_ref() {
        let theme_root = zola_root.join("themes").join(theme);
        apply_virtual_file_projection(
            &root,
            &theme_root.join("templates"),
            Some(&["html"]),
            draft_sources,
            deleted_sources,
            &mut theme_template_files,
        )?;
        apply_virtual_file_projection(
            &root,
            &theme_root.join("sass"),
            Some(&["scss"]),
            draft_sources,
            deleted_sources,
            &mut theme_style_files,
        )?;
        apply_virtual_file_projection(
            &root,
            &theme_root.join("static"),
            Some(&["css", "scss"]),
            draft_sources,
            deleted_sources,
            &mut theme_style_files,
        )?;
        apply_virtual_file_projection(
            &root,
            &theme_root.join("static"),
            None,
            draft_sources,
            deleted_sources,
            &mut theme_asset_files,
        )?;
    }
    if let Some(projection) = workspace_projection {
        add_workspace_manifest_only_paths(
            &root,
            active_theme.as_deref(),
            projection,
            &mut asset_files,
            &mut theme_asset_files,
            &mut data_file_paths,
        )?;
    }

    let mut templates = Vec::new();
    for path in template_files {
        templates.push(scan_template(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Local,
            None,
            draft_sources,
            &mut builder,
        ));
    }
    for path in theme_template_files {
        templates.push(scan_template(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Theme,
            active_theme.clone(),
            draft_sources,
            &mut builder,
        ));
    }

    let mut styles = Vec::new();
    for path in style_files {
        styles.push(scan_style(
            &root,
            &path,
            crate::source_graph::model::SourceOrigin::Local,
            None,
            &mut builder,
        ));
    }
    for path in theme_style_files {
        styles.push(scan_style(
            &root,
            &path,
            crate::source_graph::model::SourceOrigin::Theme,
            active_theme.clone(),
            &mut builder,
        ));
    }
    let mut assets = Vec::new();
    for path in asset_files {
        assets.push(scan_asset(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Local,
            None,
            &mut builder,
        ));
    }
    for path in theme_asset_files {
        assets.push(scan_asset(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Theme,
            active_theme.clone(),
            &mut builder,
        ));
    }
    let mut data_files = Vec::new();
    for path in data_file_paths {
        data_files.push(scan_data_file(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Local,
            None,
            &mut builder,
        ));
    }

    let template_node_by_name = template_node_map(&templates);
    let template_by_name = template_summary_map(&templates);
    let block_node_by_template_and_name = block_node_map(&templates);
    let style_by_file: HashMap<String, String> = styles
        .iter()
        .map(|style| (style.file.clone(), style.node_id.clone()))
        .collect();
    let asset_node_by_reference = asset_reference_map(&assets);
    let data_file_node_by_reference = data_file_reference_map(&data_files);
    add_template_relations(
        &templates,
        &template_node_by_name,
        &block_node_by_template_and_name,
        &mut builder,
    );
    add_template_style_relations(&templates, &style_by_file, &theme_resolver, &mut builder);
    add_template_asset_relations(&templates, &asset_node_by_reference, &mut builder);

    let mut pages = Vec::new();
    for path in content_files {
        pages.push(scan_content_page(
            &root,
            &zola_root,
            &path,
            &template_node_by_name,
            &template_by_name,
            &style_by_file,
            &theme_resolver,
            draft_sources,
            &mut builder,
        ));
    }
    let content_node_by_path = content_node_map(&pages);
    add_template_load_data_relations(
        &templates,
        &asset_node_by_reference,
        &data_file_node_by_reference,
        &content_node_by_path,
        &mut builder,
    );
    add_template_content_relations(&templates, &pages, &mut builder);

    let graph_templates = templates
        .into_iter()
        .map(|template| SourceGraphTemplate {
            id: template.id,
            file: template.file,
            name: template.name,
            origin: template.origin,
            theme_name: template.theme_name,
            is_partial: template.is_partial,
            extends: template.extends,
            includes: template.includes,
            imports: template.imports,
            get_pages: template.get_pages,
            get_sections: template.get_sections,
            internal_links: template.internal_links,
            asset_urls: template.asset_urls,
            asset_hashes: template.asset_hashes,
            data_loads: template.data_loads,
            image_metadata: template.image_metadata,
            image_resizes: template.image_resizes,
            blocks: template
                .blocks
                .into_iter()
                .map(|(block, _node_id)| block)
                .collect(),
            macros: template.macros,
            node_id: template.node_id,
        })
        .collect();
    let graph_styles = styles
        .into_iter()
        .map(|style| SourceGraphStyle {
            id: style.node_id.clone(),
            file: style.file.clone(),
            origin: style.origin,
            theme_name: style.theme_name,
            scope: style_scope_for_file(&style.file),
            node_id: style.node_id,
        })
        .collect();
    let graph_assets = assets
        .iter()
        .map(|asset| SourceGraphAsset {
            id: asset.node_id.clone(),
            file: asset.file.clone(),
            origin: asset.origin.clone(),
            theme_name: asset.theme_name.clone(),
            logical_path: asset.logical_path.clone(),
            node_id: asset.node_id.clone(),
        })
        .collect::<Vec<_>>();
    let graph_scripts = assets
        .into_iter()
        .filter(|asset| asset.is_script)
        .map(|script| SourceGraphScript {
            id: script.node_id.clone(),
            file: script.file,
            origin: script.origin,
            theme_name: script.theme_name,
            logical_path: script.logical_path,
            node_id: script.node_id,
        })
        .collect();
    let graph_data_files = data_files
        .into_iter()
        .map(|data_file| SourceGraphDataFile {
            id: data_file.node_id.clone(),
            file: data_file.file.clone(),
            origin: data_file.origin,
            theme_name: data_file.theme_name,
            logical_path: data_file.logical_path,
            node_id: data_file.node_id,
        })
        .collect();

    let graph = builder.finish(
        pages,
        graph_templates,
        graph_styles,
        graph_scripts,
        graph_assets,
        graph_data_files,
    );
    let read_errors = graph
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, SourceDiagnosticSeverity::Error))
        .map(|diagnostic| {
            diagnostic
                .file
                .as_ref()
                .map(|file| format!("{file}: {}", diagnostic.message))
                .unwrap_or_else(|| diagnostic.message.clone())
        })
        .collect::<Vec<_>>();
    if !read_errors.is_empty() {
        return Err(format!(
            "Source Graph a refuzat snapshotul cu erori de citire: {}",
            read_errors.join(" | ")
        ));
    }
    Ok(graph)
}

fn add_workspace_manifest_only_paths(
    project_root: &Path,
    active_theme: Option<&str>,
    projection: &WorkspaceProjectionLease,
    local_assets: &mut Vec<std::path::PathBuf>,
    theme_assets: &mut Vec<std::path::PathBuf>,
    data_files: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    projection
        .accepted_disk
        .require_identity(&projection.runtime_session_id, &projection.project_root)?;
    let projected_paths = projection
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .chain(projection.resource_bytes.keys().map(String::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    for projected_path in projected_paths {
        if projection.deleted_sources.contains(projected_path) {
            continue;
        }
        let relative = projected_path.replace('\\', "/");
        if relative.starts_with('/')
            || relative
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err(format!(
                "Source Graph a refuzat path-ul manifest nesigur {}.",
                projected_path
            ));
        }
        let path = project_root.join(&relative);
        if relative.starts_with("sursa/static/") {
            local_assets.push(path);
        } else if active_theme
            .is_some_and(|theme| relative.starts_with(&format!("sursa/themes/{theme}/static/")))
        {
            theme_assets.push(path);
        } else if relative.starts_with("sursa/date/")
            && Path::new(&relative)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    ZOLA_DATA_FILE_EXTENSIONS
                        .iter()
                        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
                })
        {
            data_files.push(path);
        }
    }
    local_assets.sort();
    local_assets.dedup();
    theme_assets.sort();
    theme_assets.dedup();
    data_files.sort();
    data_files.dedup();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::source_graph::model::{
        SourceNodeKind, SourceOrigin, SourceRelationKind, SourceStyleScope,
    };

    use super::*;

    #[test]
    fn source_graph_includes_draft_only_template_for_atomic_workspace_planning() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::write(root.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            root.join("sursa/content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/index.html"),
            "{% block content %}<main></main>{% endblock %}\n",
        )
        .unwrap();
        let drafts = HashMap::from([(
            "sursa/templates/partials/hero.html".to_string(),
            "<section class=\"hero\"></section>\n".to_string(),
        )]);

        let graph = build_source_graph_with_drafts(&root, &drafts).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert!(graph
            .templates
            .iter()
            .any(|template| template.name == "partials/hero.html" && template.is_partial));
        assert!(graph.nodes.iter().any(|node| {
            node.file == "sursa/templates/partials/hero.html"
                && node.kind == SourceNodeKind::Html
                && node.label == "<section .hero>"
        }));
    }

    #[test]
    fn source_graph_rejects_unsafe_virtual_draft_path() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::write(root.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        let drafts =
            HashMap::from([("../outside.html".to_string(), "<main></main>\n".to_string())]);

        let error = match build_source_graph_with_drafts(&root, &drafts) {
            Ok(_) => panic!("unsafe draft path should be rejected"),
            Err(error) => error,
        };
        fs::remove_dir_all(&root).unwrap();

        assert!(error.contains("path-ul draft nesigur"));
    }

    #[test]
    fn builds_minimal_zola_source_graph() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/templates/partials")).unwrap();
        fs::create_dir_all(root.join("sursa/sass/pagini")).unwrap();
        fs::create_dir_all(root.join("sursa/sass/partials")).unwrap();

        fs::write(
            root.join("sursa/config.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n\nSalut\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/base.html"),
            "<body>{% block content %}{% endblock %}</body>",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/index.html"),
            "{% extends \"base.html\" %}{% block content %}{% include \"partials/header.html\" %}{% set cards = section.extra.cards %}<main class=\"hero\"></main>{% for card in cards %}<article class=\"card\"></article>{% endfor %}{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/partials/header.html"),
            "<header></header>",
        )
        .unwrap();
        fs::write(root.join("sursa/sass/pagini/index.scss"), ".hero {}\n").unwrap();
        fs::write(root.join("sursa/sass/partials/_header.scss"), "header {}\n").unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(graph.pages.len(), 1);
        assert!(graph.templates.iter().any(|template| {
            template.name == "index.html" && template.extends.as_deref() == Some("base.html")
        }));
        assert!(graph.templates.iter().any(|template| {
            template.name == "index.html"
                && template
                    .includes
                    .contains(&"partials/header.html".to_string())
        }));
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.kind == SourceRelationKind::PageTemplate));
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.kind == SourceRelationKind::Extends));
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.kind == SourceRelationKind::Includes));
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.kind == SourceRelationKind::UsesStyle));
        let header_template = graph
            .templates
            .iter()
            .find(|template| template.name == "partials/header.html")
            .unwrap();
        let header_style = graph
            .styles
            .iter()
            .find(|style| style.file == "sursa/sass/partials/_header.scss")
            .unwrap();
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::UsesStyle
                && relation.from == header_template.node_id
                && relation.to == header_style.node_id
        }));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == SourceNodeKind::Html && node.label == "<main .hero>"));
        let main_node = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == "<main .hero>")
            .unwrap();
        let main_parent = graph
            .nodes
            .iter()
            .find(|node| Some(node.id.as_str()) == main_node.parent.as_deref())
            .unwrap();
        assert!(main_parent.kind == SourceNodeKind::Block);

        let card_node = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == "<article .card>")
            .unwrap();
        let card_parent = graph
            .nodes
            .iter()
            .find(|node| Some(node.id.as_str()) == card_node.parent.as_deref())
            .unwrap();
        assert!(card_parent.kind == SourceNodeKind::For);
        assert!(!card_node.capabilities.can_edit_visual);
    }

    #[test]
    fn section_page_template_creates_source_graph_relation() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content/blog")).unwrap();
        fs::create_dir_all(root.join("sursa/templates/blog")).unwrap();

        fs::write(
            root.join("sursa/zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/blog/_index.md"),
            "+++\ntitle = \"Blog\"\npage_template = \"blog/page.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/section.html"),
            "<h1>{{ section.title }}</h1>",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/blog/page.html"),
            "<h1>{{ page.title }}</h1>",
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let section = graph
            .pages
            .iter()
            .find(|page| page.file == "sursa/content/blog/_index.md")
            .unwrap();
        assert_eq!(
            section.frontmatter_page_template.as_deref(),
            Some("blog/page.html")
        );
        assert!(section.page_template_node_id.is_some());
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::SectionPageTemplate
                && relation.from == section.content_node_id
                && Some(&relation.to) == section.page_template_node_id.as_ref()
        }));
    }

    #[test]
    fn zola_content_functions_create_source_graph_relations() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content/blog")).unwrap();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();

        fs::write(
            root.join("sursa/zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/blog/_index.md"),
            "+++\ntitle = \"Blog\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/blog/post.md"),
            "+++\ntitle = \"Post\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/index.html"),
            r#"{% set post = get_page(path="blog/post.md") %}
{% set blog = get_section(path="blog/_index.md", metadata_only=true) %}
<a href="{{ get_url(path="@/blog/post.md") }}">Post</a>
<link rel="stylesheet" href="{{ get_url(path="css/site.css") }}">
"#,
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let template = graph
            .templates
            .iter()
            .find(|template| template.name == "index.html")
            .unwrap();
        let post = graph
            .pages
            .iter()
            .find(|page| page.file == "sursa/content/blog/post.md")
            .unwrap();
        let section = graph
            .pages
            .iter()
            .find(|page| page.file == "sursa/content/blog/_index.md")
            .unwrap();
        assert!(template.get_pages.contains(&"blog/post.md".to_string()));
        assert!(template
            .get_sections
            .contains(&"blog/_index.md".to_string()));
        assert!(template
            .internal_links
            .contains(&"blog/post.md".to_string()));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::GetsPage
                && relation.from == template.node_id
                && relation.to == post.content_node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::GetsSection
                && relation.from == template.node_id
                && relation.to == section.content_node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::InternalContentLink
                && relation.from == template.node_id
                && relation.to == post.content_node_id
        }));
        assert!(!graph
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("css/site.css")));
    }

    #[test]
    fn zola_static_asset_functions_create_source_graph_relations() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::create_dir_all(root.join("sursa/static/js")).unwrap();
        fs::create_dir_all(root.join("sursa/static/css")).unwrap();
        fs::create_dir_all(root.join("sursa/static/data")).unwrap();
        fs::create_dir_all(root.join("sursa/static/img")).unwrap();

        fs::write(
            root.join("sursa/config.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n",
        )
        .unwrap();
        fs::write(root.join("sursa/static/js/app.js"), "console.log('ok');").unwrap();
        fs::write(root.join("sursa/static/css/site.css"), "body{}").unwrap();
        fs::write(root.join("sursa/static/data/catalog.json"), "{}").unwrap();
        fs::write(root.join("sursa/static/img/hero.png"), b"png").unwrap();
        fs::write(
            root.join("sursa/templates/index.html"),
            r#"<script src="{{ get_url(path="js/app.js") }}" integrity="{{ get_hash(path="static/js/app.js") }}"></script>
<link rel="stylesheet" href="{{ get_url(path="css/site.css") }}">
{% set data = load_data(path="static/data/catalog.json") %}
{% set meta = get_image_metadata(path="static/img/hero.png") %}
{% set image = resize_image(path="static/img/hero.png", width=640, op="fit_width") %}
"#,
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let template = graph
            .templates
            .iter()
            .find(|template| template.name == "index.html")
            .unwrap();
        let script = graph
            .assets
            .iter()
            .find(|asset| asset.logical_path == "js/app.js")
            .unwrap();
        let stylesheet = graph
            .assets
            .iter()
            .find(|asset| asset.logical_path == "css/site.css")
            .unwrap();
        let data = graph
            .assets
            .iter()
            .find(|asset| asset.logical_path == "data/catalog.json")
            .unwrap();
        let image = graph
            .assets
            .iter()
            .find(|asset| asset.logical_path == "img/hero.png")
            .unwrap();

        assert!(template.asset_urls.contains(&"js/app.js".to_string()));
        assert!(template
            .asset_hashes
            .contains(&"static/js/app.js".to_string()));
        assert!(template
            .data_loads
            .contains(&"static/data/catalog.json".to_string()));
        assert!(template
            .image_metadata
            .contains(&"static/img/hero.png".to_string()));
        assert!(template
            .image_resizes
            .contains(&"static/img/hero.png".to_string()));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::AssetUrl
                && relation.from == template.node_id
                && relation.to == script.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::AssetHash
                && relation.from == template.node_id
                && relation.to == script.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::AssetUrl
                && relation.from == template.node_id
                && relation.to == stylesheet.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::DataLoad
                && relation.from == template.node_id
                && relation.to == data.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::ImageMetadata
                && relation.from == template.node_id
                && relation.to == image.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::ImageResize
                && relation.from == template.node_id
                && relation.to == image.node_id
        }));
    }

    #[test]
    fn zola_data_files_create_load_data_relations() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::create_dir_all(root.join("sursa/date")).unwrap();

        fs::write(
            root.join("sursa/zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/date/meniu.toml"),
            "[[item]]\nlabel = \"Acasă\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/index.html"),
            r#"{% set meniu = load_data(path="date/meniu.toml") %}"#,
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let template = graph
            .templates
            .iter()
            .find(|template| template.name == "index.html")
            .unwrap();
        let data_file = graph
            .data_files
            .iter()
            .find(|data_file| data_file.logical_path == "date/meniu.toml")
            .unwrap();

        assert!(template.data_loads.contains(&"date/meniu.toml".to_string()));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == SourceNodeKind::DataFile
                && node.file == "sursa/date/meniu.toml"));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::DataFileLoad
                && relation.from == template.node_id
                && relation.to == data_file.node_id
        }));
    }

    #[test]
    fn zola_content_files_create_load_data_relations() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content/blog")).unwrap();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();

        fs::write(
            root.join("sursa/zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/blog/post.md"),
            "+++\ntitle = \"Post\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/index.html"),
            r#"{% set post = load_data(path="@/blog/post.md") %}
{% set post_copy = load_data(path="content/blog/post.md") %}
"#,
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let template = graph
            .templates
            .iter()
            .find(|template| template.name == "index.html")
            .unwrap();
        let page = graph
            .pages
            .iter()
            .find(|page| page.file == "sursa/content/blog/post.md")
            .unwrap();

        assert!(template.data_loads.contains(&"@/blog/post.md".to_string()));
        assert!(template
            .data_loads
            .contains(&"content/blog/post.md".to_string()));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::ContentDataLoad
                && relation.from == template.node_id
                && relation.to == page.content_node_id
        }));
    }

    #[test]
    fn resolves_active_theme_templates_as_fallback() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/themes/test-theme/templates/partials")).unwrap();
        fs::create_dir_all(root.join("sursa/themes/test-theme/static/css")).unwrap();
        fs::create_dir_all(root.join("sursa/themes/test-theme/sass/pagini")).unwrap();

        fs::write(
            root.join("sursa/zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n\nSalut\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/themes/test-theme/templates/index.html"),
            "{% extends \"base.html\" %}{% block content %}<main></main>{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("sursa/themes/test-theme/templates/base.html"),
            "<body>{% include \"partials/footer.html\" %}{% block content %}{% endblock %}</body>",
        )
        .unwrap();
        fs::write(
            root.join("sursa/themes/test-theme/templates/partials/footer.html"),
            "<footer></footer>",
        )
        .unwrap();
        fs::write(
            root.join("sursa/themes/test-theme/static/css/style.css"),
            "body { color: black; }",
        )
        .unwrap();
        fs::write(
            root.join("sursa/themes/test-theme/sass/pagini/index.scss"),
            ".theme-main { color: red; }",
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(graph.active_theme.as_deref(), Some("test-theme"));
        let page = graph.pages.iter().find(|page| page.url == "/").unwrap();
        let template = page
            .template_node_id
            .as_ref()
            .and_then(|node_id| {
                graph
                    .templates
                    .iter()
                    .find(|template| &template.node_id == node_id)
            })
            .unwrap();
        assert_eq!(template.name, "index.html");
        assert_eq!(template.origin, SourceOrigin::Theme);
        assert_eq!(template.theme_name.as_deref(), Some("test-theme"));
        assert!(graph.templates.iter().any(|template| {
            template.name == "partials/footer.html" && template.origin == SourceOrigin::Theme
        }));
        assert!(graph.styles.iter().any(|style| {
            style.file == "sursa/themes/test-theme/static/css/style.css"
                && style.origin == SourceOrigin::Theme
                && matches!(style.scope, SourceStyleScope::Global)
        }));
        let theme_page_style = graph
            .styles
            .iter()
            .find(|style| style.file == "sursa/themes/test-theme/sass/pagini/index.scss")
            .unwrap();
        assert!(graph.relations.iter().any(|relation| {
            relation.from == page.id
                && relation.to == theme_page_style.node_id
                && relation.kind == SourceRelationKind::UsesStyle
        }));
    }

    #[test]
    fn local_template_overrides_theme_template_for_page_style() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::create_dir_all(root.join("sursa/sass/pagini")).unwrap();
        fs::create_dir_all(root.join("sursa/themes/test-theme/templates")).unwrap();
        fs::create_dir_all(root.join("sursa/themes/test-theme/sass/pagini")).unwrap();

        fs::write(
            root.join("sursa/zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n\nSalut\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/index.html"),
            "{% block content %}<main class=\"local\"></main>{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("sursa/sass/pagini/index.scss"),
            ".local { color: blue; }",
        )
        .unwrap();
        fs::write(
            root.join("sursa/themes/test-theme/templates/index.html"),
            "{% block content %}<main class=\"theme\"></main>{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("sursa/themes/test-theme/sass/pagini/index.scss"),
            ".theme { color: red; }",
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let page = graph.pages.iter().find(|page| page.url == "/").unwrap();
        let template = page
            .template_node_id
            .as_ref()
            .and_then(|node_id| {
                graph
                    .templates
                    .iter()
                    .find(|template| &template.node_id == node_id)
            })
            .unwrap();
        assert_eq!(template.origin, SourceOrigin::Local);

        let local_page_style = graph
            .styles
            .iter()
            .find(|style| style.file == "sursa/sass/pagini/index.scss")
            .unwrap();
        assert!(graph.relations.iter().any(|relation| {
            relation.from == page.id
                && relation.to == local_page_style.node_id
                && relation.kind == SourceRelationKind::UsesStyle
        }));
    }

    #[test]
    fn partial_blocks_are_diagnostics_not_layout_blocks() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/templates/partials")).unwrap();

        fs::write(
            root.join("sursa/config.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/index.html"),
            "{% include \"partials/cta.html\" %}",
        )
        .unwrap();
        fs::write(
            root.join("sursa/templates/partials/cta.html"),
            "{% block content %}<section class=\"cta\"></section>{% endblock %}",
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let partial = graph
            .templates
            .iter()
            .find(|template| template.name == "partials/cta.html")
            .unwrap();
        assert!(partial.is_partial);
        assert!(partial.blocks.is_empty());
        assert!(graph.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("Partialul partials/cta.html conține block Tera")
        }));
        assert!(!graph.nodes.iter().any(|node| {
            node.file.ends_with("templates/partials/cta.html") && node.kind == SourceNodeKind::Block
        }));
        let section = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == "<section .cta>")
            .unwrap();
        let section_parent = graph
            .nodes
            .iter()
            .find(|node| Some(node.id.as_str()) == section.parent.as_deref())
            .unwrap();
        assert!(matches!(section_parent.kind, SourceNodeKind::Partial));
    }

    fn unique_test_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-source-graph-{nanos}"))
    }
}

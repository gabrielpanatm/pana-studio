use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use toml_edit::{value, DocumentMut, Item, Table};

use crate::{
    kernel::{
        file_buffer_store::{require_file_buffer_session_binding, FileBufferRequestIdentity},
        observability::now_ms,
        project_path::normalize_project_relative_path,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspace, ProjectWorkspaceIdentity,
            ProjectWorkspaceMutationReceipt, ProjectWorkspaceSnapshot, WorkspaceMutationMetadata,
            WorkspaceResourceMutation,
        },
    },
    project::{build_content_page_draft_with_active_theme, zola_project_root},
    source_graph::zola::zola_frontmatter_range,
    state::AppState,
    zola_theme::active_theme_from_source,
};

pub const WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntryMutationReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub relative_path: Option<String>,
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PageFrontmatterField {
    Title,
    Description,
    Date,
    Template,
    Slug,
    Weight,
    PaginateBy,
    Draft,
    Hidden,
    IncludeInFeeds,
    SeoTitle,
    SeoDescription,
    CanonicalUrl,
    Robots,
    OgTitle,
    OgDescription,
    OgImage,
    OgType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum PageFrontmatterScalar {
    String(String),
    Integer(i64),
    Boolean(bool),
    Empty,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageFrontmatterFieldUpdateInput {
    pub relative_path: String,
    pub field: PageFrontmatterField,
    pub value: PageFrontmatterScalar,
}

#[derive(Clone, Copy)]
enum PageFrontmatterKey {
    Root(&'static str),
    Extra(&'static str),
}

fn page_frontmatter_key(field: PageFrontmatterField) -> PageFrontmatterKey {
    match field {
        PageFrontmatterField::Title => PageFrontmatterKey::Root("title"),
        PageFrontmatterField::Description => PageFrontmatterKey::Root("description"),
        PageFrontmatterField::Date => PageFrontmatterKey::Root("date"),
        PageFrontmatterField::Template => PageFrontmatterKey::Root("template"),
        PageFrontmatterField::Slug => PageFrontmatterKey::Root("slug"),
        PageFrontmatterField::Weight => PageFrontmatterKey::Root("weight"),
        PageFrontmatterField::PaginateBy => PageFrontmatterKey::Root("paginate_by"),
        PageFrontmatterField::Draft => PageFrontmatterKey::Root("draft"),
        PageFrontmatterField::Hidden => PageFrontmatterKey::Root("hidden"),
        PageFrontmatterField::IncludeInFeeds => PageFrontmatterKey::Root("include_in_feeds"),
        PageFrontmatterField::SeoTitle => PageFrontmatterKey::Extra("seo_title"),
        PageFrontmatterField::SeoDescription => PageFrontmatterKey::Extra("seo_description"),
        PageFrontmatterField::CanonicalUrl => PageFrontmatterKey::Extra("canonical_url"),
        PageFrontmatterField::Robots => PageFrontmatterKey::Extra("robots"),
        PageFrontmatterField::OgTitle => PageFrontmatterKey::Extra("og_title"),
        PageFrontmatterField::OgDescription => PageFrontmatterKey::Extra("og_description"),
        PageFrontmatterField::OgImage => PageFrontmatterKey::Extra("og_image"),
        PageFrontmatterField::OgType => PageFrontmatterKey::Extra("og_type"),
    }
}

fn validated_page_frontmatter_item(
    field: PageFrontmatterField,
    scalar: PageFrontmatterScalar,
) -> Result<Option<Item>, String> {
    match (field, scalar) {
        (PageFrontmatterField::Weight, PageFrontmatterScalar::Integer(number)) if number >= 0 => {
            Ok(Some(value(number)))
        }
        (PageFrontmatterField::Weight, PageFrontmatterScalar::Empty) => Ok(None),
        (PageFrontmatterField::Weight, PageFrontmatterScalar::Integer(_)) => {
            Err("Ordinea paginii trebuie să fie un număr întreg pozitiv sau zero.".to_string())
        }
        (PageFrontmatterField::Weight, _) => Err(
            "Ordinea paginii trebuie trimisă către Rust ca număr întreg, nu ca text.".to_string(),
        ),
        (PageFrontmatterField::PaginateBy, PageFrontmatterScalar::Integer(number))
            if number > 0 =>
        {
            Ok(Some(value(number)))
        }
        (PageFrontmatterField::PaginateBy, PageFrontmatterScalar::Integer(_)) => {
            Err("Arhiva trebuie să conțină cel puțin un articol pe pagină.".to_string())
        }
        (PageFrontmatterField::PaginateBy, _) => Err(
            "Numărul de articole pe pagină trebuie trimis către Rust ca număr întreg pozitiv."
                .to_string(),
        ),
        (
            PageFrontmatterField::Draft | PageFrontmatterField::Hidden,
            PageFrontmatterScalar::Boolean(enabled),
        ) => Ok(Some(value(enabled))),
        (PageFrontmatterField::Hidden, PageFrontmatterScalar::Empty) => Ok(None),
        (PageFrontmatterField::Draft, _) => {
            Err("Starea draft trebuie trimisă către Rust ca valoare booleană.".to_string())
        }
        (PageFrontmatterField::Hidden, _) => Err(
            "Vizibilitatea trebuie trimisă către Rust ca boolean sau stare moștenită.".to_string(),
        ),
        (PageFrontmatterField::IncludeInFeeds, PageFrontmatterScalar::Boolean(false)) => {
            Ok(Some(value(false)))
        }
        (PageFrontmatterField::IncludeInFeeds, PageFrontmatterScalar::Empty) => Ok(None),
        (PageFrontmatterField::IncludeInFeeds, _) => Err(
            "include_in_feeds folosește false explicit sau absența pentru default-ul true."
                .to_string(),
        ),
        (PageFrontmatterField::Title, PageFrontmatterScalar::String(text))
            if text.trim().is_empty() =>
        {
            Err("Titlul paginii nu poate fi gol.".to_string())
        }
        (PageFrontmatterField::Title, PageFrontmatterScalar::Empty) => {
            Err("Titlul paginii nu poate fi gol.".to_string())
        }
        (_, PageFrontmatterScalar::String(text)) if text.trim().is_empty() => Ok(None),
        (_, PageFrontmatterScalar::String(text)) => Ok(Some(value(text))),
        (_, PageFrontmatterScalar::Empty) => Ok(None),
        (_, _) => Err("Tipul valorii nu corespunde câmpului de frontmatter.".to_string()),
    }
}

fn set_page_frontmatter_item(
    document: &mut DocumentMut,
    key: PageFrontmatterKey,
    item: Option<Item>,
) -> Result<(), String> {
    match key {
        PageFrontmatterKey::Root(key) => {
            if let Some(item) = item {
                document[key] = item;
            } else {
                document.as_table_mut().remove(key);
            }
        }
        PageFrontmatterKey::Extra(key) => {
            if item.is_none() {
                if let Some(extra) = document.as_table_mut().get_mut("extra") {
                    let table = extra.as_table_like_mut().ok_or_else(|| {
                        "Frontmatter-ul folosește `extra` cu un tip care nu poate primi metadate SEO."
                            .to_string()
                    })?;
                    table.remove(key);
                    if table.is_empty() {
                        document.as_table_mut().remove("extra");
                    }
                }
                return Ok(());
            }

            if !document.as_table().contains_key("extra") {
                document["extra"] = Item::Table(Table::new());
            }
            let extra = document["extra"].as_table_like_mut().ok_or_else(|| {
                "Frontmatter-ul folosește `extra` cu un tip care nu poate primi metadate SEO."
                    .to_string()
            })?;
            extra.insert(key, item.expect("item verificat mai sus"));
        }
    }
    Ok(())
}

fn rewrite_page_frontmatter_field(
    source: &str,
    field: PageFrontmatterField,
    scalar: PageFrontmatterScalar,
) -> Result<String, String> {
    let trimmed = source.trim_start_matches('\u{feff}');
    if trimmed.starts_with("---") {
        return Err(
            "Setările vizuale nu rescriu frontmatter YAML; deschide documentul în Cod.".to_string(),
        );
    }

    let item = validated_page_frontmatter_item(field, scalar)?;
    let key = page_frontmatter_key(field);
    if trimmed.starts_with("+++") {
        let (start, end) = zola_frontmatter_range(source)
            .ok_or_else(|| "Pagina nu are frontmatter TOML delimitat valid.".to_string())?;
        let mut document = source[start..end]
            .parse::<DocumentMut>()
            .map_err(|error| format!("Frontmatter TOML invalid: {error}"))?;
        set_page_frontmatter_item(&mut document, key, item)?;
        let rendered_document = document.to_string();
        rendered_document
            .parse::<DocumentMut>()
            .map_err(|error| format!("Frontmatter TOML rezultat invalid: {error}"))?;
        let rendered = if rendered_document.starts_with('\n') {
            rendered_document
        } else {
            format!("\n{}", rendered_document.trim_end())
        };
        let mut next = source.to_string();
        next.replace_range(start..end, &rendered);
        return Ok(next);
    }

    if item.is_none() {
        return Ok(source.to_string());
    }
    let mut document = DocumentMut::new();
    set_page_frontmatter_item(&mut document, key, item)?;
    let rendered = document.to_string();
    Ok(format!("+++\n{}+++\n\n{}", rendered, source))
}

pub(crate) fn current_workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

pub(crate) fn mutation_metadata(label: &str, source: &str) -> WorkspaceMutationMetadata {
    WorkspaceMutationMetadata {
        label: label.to_string(),
        source: source.to_string(),
        coalesce_key: None,
        transaction_id: None,
    }
}

pub(crate) fn require_bound_workspace<'a>(
    state: &'a AppState,
    identity: &FileBufferRequestIdentity,
) -> Result<(PathBuf, std::sync::MutexGuard<'a, Option<ProjectWorkspace>>), String> {
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru operația de workspace.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există proiect curent pentru operația de workspace.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru operația de fișier.".to_string())?;
    let live = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let root_string = root.to_string_lossy().into_owned();
    require_file_buffer_session_binding(&root_string, &live.session, &live.documents, identity)?;
    live.accepted_disk.require_live_complete(
        &live.runtime_session_id(),
        &live.session.project_root,
        &root,
    )?;
    Ok((root, workspace))
}

pub(crate) fn finish_mutation(
    app: &AppHandle,
    workspace: &mut ProjectWorkspace,
    relative_path: Option<String>,
    mutate: impl FnOnce(&mut ProjectWorkspace) -> Result<ProjectWorkspaceMutationReceipt, String>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let mutation = commit_project_workspace_session_mutation(app, workspace, mutate)?;
    Ok(WorkspaceEntryMutationReceipt {
        schema_version: WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        relative_path,
        mutation,
        workspace: workspace.snapshot(),
    })
}

#[tauri::command(async)]
pub fn workspace_create_project_text_file(
    relative_path: String,
    contents: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let relative_path = normalize_project_relative_path(&relative_path)?;
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare fișier", "files.create"),
            vec![WorkspaceResourceMutation {
                relative_path,
                contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_create_content_page(
    section: String,
    slug: String,
    title: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let active_theme = ["zola.toml", "config.toml"]
        .iter()
        .find_map(|path| workspace.documents.text_for(path))
        .and_then(|source| active_theme_from_source(&source));
    let draft = build_content_page_draft_with_active_theme(
        &zola_project_root(&root),
        &section,
        &slug,
        &title,
        active_theme,
    )?;
    let relative_path = draft.relative_path;
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare pagină", "pages.create"),
            vec![WorkspaceResourceMutation {
                relative_path,
                contents: draft.contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_update_page_frontmatter_field(
    input: PageFrontmatterFieldUpdateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let relative_path = normalize_project_relative_path(&input.relative_path)?;
    if !relative_path.starts_with("content/") || !relative_path.ends_with(".md") {
        return Err(
            "Setările paginii pot modifica doar documente Markdown din `content/`.".to_string(),
        );
    }
    let is_section = relative_path.ends_with("/_index.md");
    if input.field == PageFrontmatterField::PaginateBy
        && (!is_section || relative_path == "content/_index.md")
    {
        return Err(
            "Paginarea poate fi configurată doar pentru o secțiune Zola, nu pentru o pagină."
                .to_string(),
        );
    }
    if input.field == PageFrontmatterField::IncludeInFeeds && is_section {
        return Err("include_in_feeds poate fi configurat numai pentru pagini Zola.".to_string());
    }

    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let source = workspace
        .documents
        .text_for(&relative_path)
        .ok_or_else(|| format!("Documentul `{relative_path}` nu există în ProjectWorkspace."))?;
    let contents = rewrite_page_frontmatter_field(&source, input.field, input.value)?;
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata(
                "Actualizare frontmatter pagină",
                "page_settings.frontmatter.typed",
            ),
            vec![WorkspaceResourceMutation {
                relative_path,
                contents,
                create_only: false,
            }],
            now_ms(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed_toml_frontmatter(source: &str) -> DocumentMut {
        let (start, end) = zola_frontmatter_range(source).expect("frontmatter delimitat");
        source[start..end]
            .parse::<DocumentMut>()
            .expect("frontmatter TOML valid")
    }

    #[test]
    fn updating_title_preserves_numeric_weight_and_unknown_fields() {
        let source = r#"+++
title = "Serviciu"
weight = 1
custom = "păstrat"
+++

Conținut
"#;
        let updated = rewrite_page_frontmatter_field(
            source,
            PageFrontmatterField::Title,
            PageFrontmatterScalar::String("Serviciu actualizat".to_string()),
        )
        .expect("actualizare validă");
        let document = parsed_toml_frontmatter(&updated);

        assert_eq!(document["title"].as_str(), Some("Serviciu actualizat"));
        assert_eq!(document["weight"].as_integer(), Some(1));
        assert_eq!(document["custom"].as_str(), Some("păstrat"));
        assert!(updated.ends_with("Conținut\n"));
    }

    #[test]
    fn updating_weight_repairs_legacy_quoted_value_as_integer() {
        let source = "+++\ntitle = \"Gazduire\"\nweight = \"2\"\n+++\n";
        let updated = rewrite_page_frontmatter_field(
            source,
            PageFrontmatterField::Weight,
            PageFrontmatterScalar::Integer(2),
        )
        .expect("actualizare validă");
        let document = parsed_toml_frontmatter(&updated);

        assert_eq!(document["weight"].as_integer(), Some(2));
        assert_eq!(document["weight"].as_str(), None);
    }

    #[test]
    fn updating_section_pagination_preserves_other_frontmatter() {
        let source =
            "+++\ntitle = \"Servicii\"\ntemplate = \"servicii/arhiva.html\"\ncustom = \"păstrat\"\n+++\n";
        let updated = rewrite_page_frontmatter_field(
            source,
            PageFrontmatterField::PaginateBy,
            PageFrontmatterScalar::Integer(12),
        )
        .expect("actualizare validă");
        let document = parsed_toml_frontmatter(&updated);

        assert_eq!(document["paginate_by"].as_integer(), Some(12));
        assert_eq!(document["template"].as_str(), Some("servicii/arhiva.html"));
        assert_eq!(document["custom"].as_str(), Some("păstrat"));
    }

    #[test]
    fn pagination_rejects_zero_and_empty_values() {
        for scalar in [
            PageFrontmatterScalar::Integer(0),
            PageFrontmatterScalar::Empty,
        ] {
            let error = rewrite_page_frontmatter_field(
                "+++\ntitle = \"Servicii\"\n+++\n",
                PageFrontmatterField::PaginateBy,
                scalar,
            )
            .expect_err("paginarea nu poate fi dezactivată");
            assert!(error.contains("număr întreg pozitiv") || error.contains("cel puțin un"));
        }
    }

    #[test]
    fn updating_seo_preserves_existing_extra_contract() {
        let source = r#"+++
title = "Serviciu"

[extra]
custom_field = "păstrat"
+++
"#;
        let updated = rewrite_page_frontmatter_field(
            source,
            PageFrontmatterField::SeoTitle,
            PageFrontmatterScalar::String("Titlu SEO".to_string()),
        )
        .expect("actualizare validă");
        let document = parsed_toml_frontmatter(&updated);
        let extra = document["extra"].as_table().expect("tabel extra");

        assert_eq!(extra["custom_field"].as_str(), Some("păstrat"));
        assert_eq!(extra["seo_title"].as_str(), Some("Titlu SEO"));
    }

    #[test]
    fn weight_rejects_text_at_the_rust_boundary() {
        let error = rewrite_page_frontmatter_field(
            "+++\ntitle = \"Pagină\"\n+++\n",
            PageFrontmatterField::Weight,
            PageFrontmatterScalar::String("2".to_string()),
        )
        .expect_err("textul nu trebuie acceptat ca weight");

        assert!(error.contains("număr întreg"));
    }

    #[test]
    fn clearing_optional_field_removes_only_that_key() {
        let source = "+++\ntitle = \"Pagină\"\ndescription = \"Descriere\"\nweight = 3\n+++\n";
        let updated = rewrite_page_frontmatter_field(
            source,
            PageFrontmatterField::Description,
            PageFrontmatterScalar::Empty,
        )
        .expect("ștergere validă");
        let document = parsed_toml_frontmatter(&updated);

        assert!(!document.as_table().contains_key("description"));
        assert_eq!(document["weight"].as_integer(), Some(3));
    }

    #[test]
    fn hidden_round_trip_preserves_inherited_true_and_false_states() {
        let source = "+++\ntitle = \"Pagină\"\ncustom = \"păstrat\"\n+++\n";
        let hidden = rewrite_page_frontmatter_field(
            source,
            PageFrontmatterField::Hidden,
            PageFrontmatterScalar::Boolean(true),
        )
        .unwrap();
        assert_eq!(
            parsed_toml_frontmatter(&hidden)["hidden"].as_bool(),
            Some(true)
        );

        let visible = rewrite_page_frontmatter_field(
            &hidden,
            PageFrontmatterField::Hidden,
            PageFrontmatterScalar::Boolean(false),
        )
        .unwrap();
        assert_eq!(
            parsed_toml_frontmatter(&visible)["hidden"].as_bool(),
            Some(false)
        );

        let inherited = rewrite_page_frontmatter_field(
            &visible,
            PageFrontmatterField::Hidden,
            PageFrontmatterScalar::Empty,
        )
        .unwrap();
        let document = parsed_toml_frontmatter(&inherited);
        assert!(!document.as_table().contains_key("hidden"));
        assert_eq!(document["custom"].as_str(), Some("păstrat"));
    }

    #[test]
    fn include_in_feeds_writes_only_the_non_default_false_value() {
        let source = "+++\ntitle = \"Articol\"\ncustom = \"păstrat\"\n+++\n";
        let excluded = rewrite_page_frontmatter_field(
            source,
            PageFrontmatterField::IncludeInFeeds,
            PageFrontmatterScalar::Boolean(false),
        )
        .unwrap();
        assert_eq!(
            parsed_toml_frontmatter(&excluded)["include_in_feeds"].as_bool(),
            Some(false)
        );

        let defaulted = rewrite_page_frontmatter_field(
            &excluded,
            PageFrontmatterField::IncludeInFeeds,
            PageFrontmatterScalar::Empty,
        )
        .unwrap();
        let document = parsed_toml_frontmatter(&defaulted);
        assert!(!document.as_table().contains_key("include_in_feeds"));
        assert_eq!(document["custom"].as_str(), Some("păstrat"));

        assert!(rewrite_page_frontmatter_field(
            source,
            PageFrontmatterField::IncludeInFeeds,
            PageFrontmatterScalar::Boolean(true),
        )
        .unwrap_err()
        .contains("default-ul true"));
    }
}

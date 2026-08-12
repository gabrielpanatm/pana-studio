use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use toml_edit::{value, DocumentMut};

use crate::{
    commands::workspace_entries::{
        current_workspace_identity, finish_mutation, mutation_metadata, require_bound_workspace,
        WorkspaceEntryMutationReceipt,
    },
    kernel::{
        file_buffer_store::FileBufferRequestIdentity,
        listing_items::{
            listing_item_contract_entries, serialize_listing_item_contract,
            ListingItemContractEntry, LISTING_ITEM_METADATA_PATH,
        },
        observability::now_ms,
        project_path::normalize_project_relative_path,
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceMutationReceipt, WorkspaceResourceDelete,
            WorkspaceResourceMutation,
        },
        source_graph_rewrite::{
            plan_template_reference_workspace_mutation_from_graph, SourceGraphRewriteOperation,
        },
    },
    project::{DEFAULT_ARCHIVE_PAGINATE_BY, DEFAULT_ARCHIVE_PAGINATE_PATH},
    source_graph::{
        build_source_graph_from_workspace_projection, build_taxonomy_catalog,
        build_template_catalog,
        model::{SourceNodeKind, SourcePageKind},
        tera::{parse_tera_items, TeraItemKind},
        zola::{normalize_zola_template_reference, zola_frontmatter_range},
    },
    state::AppState,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateDraftRole {
    Page,
    Layout,
    Partial,
    MacroLibrary,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTemplateInput {
    pub name: String,
    pub role: TemplateDraftRole,
    #[serde(default)]
    pub parent_template_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateListingItemInput {
    pub label: String,
    pub slug: String,
    pub model_id: String,
    pub preview_page_file: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteListingItemInput {
    pub id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSemanticCreateRole {
    Layout,
    Homepage,
    DefaultPage,
    SpecificPage,
    SectionArchive,
    SectionElement,
    TaxonomyList,
    TaxonomyTerm,
    NotFound,
    Custom,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSemanticTemplateInput {
    pub role: TemplateSemanticCreateRole,
    pub name: String,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub new_section: Option<CreateSemanticSectionInput>,
    #[serde(default)]
    pub parent_template_name: Option<String>,
    #[serde(default)]
    pub include_page_content: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSemanticSectionInput {
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub sort_by: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedSemanticSection {
    title: String,
    relative_path: String,
    contents: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTemplateCollectionInput {
    pub title: String,
    pub slug: String,
    pub list_template_name: String,
    pub item_template_name: String,
    #[serde(default)]
    pub parent_template_name: Option<String>,
    #[serde(default)]
    pub include_page_content: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTemplateParentInput {
    pub relative_path: String,
    #[serde(default)]
    pub parent_template_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateAssignmentKey {
    Template,
    PageTemplate,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTemplateAssignmentInput {
    pub content_relative_path: String,
    pub key: TemplateAssignmentKey,
    #[serde(default)]
    pub template_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateTemplateInput {
    pub source_relative_path: String,
    pub destination_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideThemeTemplateInput {
    pub source_relative_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameTemplateInput {
    pub source_relative_path: String,
    pub destination_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTemplateInput {
    pub relative_path: String,
}

#[tauri::command(async)]
pub fn workspace_create_template(
    input: CreateTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let destination = local_template_path(&input.name)?;
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    require_destination_available(workspace, &destination)?;
    let parent = validate_parent_template(
        &root,
        workspace,
        input.parent_template_name.as_deref(),
        None,
    )?;
    let contents = template_draft(input.role, parent.as_deref());
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare șablon Tera", "templates.create"),
            vec![WorkspaceResourceMutation {
                relative_path: destination,
                contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_create_listing_item(
    input: CreateListingItemInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let slug = listing_item_slug(&input.slug)?;
    let label = input.label.trim();
    if label.is_empty() || label.len() > 160 || label.contains('\0') {
        return Err("Numele Listing Item-ului este gol, prea lung sau invalid.".to_string());
    }
    let template_name = format!("listing-items/{slug}.html");
    let destination = format!("templates/{template_name}");
    let item_id = slug.clone();
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    require_destination_available(workspace, &destination)?;
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    if !graph
        .content_models
        .models
        .iter()
        .any(|model| model.id == input.model_id)
    {
        return Err(format!("Modelul de conținut {} nu există.", input.model_id));
    }
    let preview_page = graph
        .pages
        .iter()
        .find(|page| page.file == input.preview_page_file)
        .ok_or_else(|| {
            format!(
                "Articolul de preview {} nu există.",
                input.preview_page_file
            )
        })?;
    if preview_page.page_kind != SourcePageKind::Page {
        return Err("Preview-ul Listing Item cere un articol, nu o secțiune.".to_string());
    }
    let preview_model = graph
        .content_models
        .page_bindings
        .iter()
        .find(|binding| binding.page_file == input.preview_page_file)
        .map(|binding| binding.model_id.as_str());
    if preview_model != Some(input.model_id.as_str()) {
        return Err(format!(
            "Articolul de preview aparține modelului {}, nu {}.",
            preview_model.unwrap_or("neatribuit"),
            input.model_id
        ));
    }
    let existing_metadata = workspace.documents.text_for(LISTING_ITEM_METADATA_PATH);
    let mut entries = listing_item_contract_entries(existing_metadata.as_deref())?;
    if entries
        .iter()
        .any(|entry| entry.id == item_id || entry.template_name == template_name)
    {
        return Err(format!("Listing Item-ul {item_id} există deja."));
    }
    entries.push(ListingItemContractEntry {
        id: item_id.clone(),
        label: label.to_string(),
        template_name: template_name.clone(),
        model_id: input.model_id,
        preview_page_file: input.preview_page_file,
    });
    let metadata = serialize_listing_item_contract(&entries)?;
    let contents = format!(
        "<article class=\"listing-item listing-item-{slug}\" data-pana-listing-item=\"{item_id}\">\n  <h2>{{{{ item.title }}}}</h2>\n</article>\n"
    );
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare Listing Item", "templates.listing_item.create"),
            vec![
                WorkspaceResourceMutation {
                    relative_path: destination,
                    contents,
                    create_only: true,
                },
                WorkspaceResourceMutation {
                    relative_path: LISTING_ITEM_METADATA_PATH.to_string(),
                    contents: metadata,
                    create_only: false,
                },
            ],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_delete_listing_item(
    input: DeleteListingItemInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    let item = graph
        .listing_items
        .items
        .iter()
        .find(|item| item.id == input.id)
        .ok_or_else(|| format!("Listing Item-ul {} nu există.", input.id))?;
    if item.usage_count > 0 {
        return Err(format!(
            "Listing Item-ul {} este folosit de {} template-uri și nu poate fi șters.",
            item.label, item.usage_count
        ));
    }
    let metadata_source = projection
        .source_texts
        .get(LISTING_ITEM_METADATA_PATH)
        .map(String::as_str);
    let mut entries = listing_item_contract_entries(metadata_source)?;
    let before_count = entries.len();
    entries.retain(|entry| entry.id != input.id);
    if entries.len() == before_count {
        return Err(format!(
            "Contractul editorial nu conține Listing Item-ul {}.",
            input.id
        ));
    }
    let metadata = serialize_listing_item_contract(&entries)?;
    let relative_path = item.file.clone();
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_changes(
            &current_workspace_identity(candidate),
            mutation_metadata("Ștergere Listing Item", "templates.listing-item.delete"),
            vec![WorkspaceResourceMutation {
                relative_path: LISTING_ITEM_METADATA_PATH.to_string(),
                contents: metadata,
                create_only: false,
            }],
            vec![WorkspaceResourceDelete { relative_path }],
            now_ms(),
        )
    })
}

fn listing_item_slug(value: &str) -> Result<String, String> {
    let value = value.trim().trim_end_matches(".html");
    if value.is_empty()
        || value.len() > 80
        || !value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
    {
        return Err(
            "Slug-ul Listing Item acceptă numai litere mici ASCII, cifre și cratime.".to_string(),
        );
    }
    Ok(value.to_string())
}

#[tauri::command(async)]
pub fn workspace_create_semantic_template(
    input: CreateSemanticTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let destination = local_template_path(&input.name)?;
    let logical_name = template_logical_name(&destination)?;
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    require_destination_available(workspace, &destination)?;

    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    let target_id = input
        .target_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if input.new_section.is_some()
        && !matches!(input.role, TemplateSemanticCreateRole::SectionArchive)
    {
        return Err(
            "O secțiune nouă poate fi creată numai împreună cu șablonul său de listă/arhivă."
                .to_string(),
        );
    }
    if input.new_section.is_some() && target_id.is_some() {
        return Err(
            "Alege fie o secțiune existentă, fie crearea unei secțiuni noi, nu ambele.".to_string(),
        );
    }
    let new_section = input
        .new_section
        .as_ref()
        .map(|section| prepare_semantic_section(section, &logical_name))
        .transpose()?;
    if let Some(section) = &new_section {
        if workspace
            .documents
            .files
            .contains_key(&section.relative_path)
        {
            return Err(format!(
                "Secțiunea {} există deja; selecteaz-o drept secțiune existentă.",
                section.relative_path
            ));
        }
    }
    let assignment = if new_section.is_some() {
        None
    } else {
        semantic_creation_assignment(workspace, &graph, input.role, target_id, &logical_name)?
    };
    let parent = validate_parent_template(
        &root,
        workspace,
        input.parent_template_name.as_deref(),
        None,
    )?;
    let draft_role = if matches!(input.role, TemplateSemanticCreateRole::Layout) {
        TemplateDraftRole::Layout
    } else {
        TemplateDraftRole::Page
    };
    let contents = if let Some(section) = &new_section {
        collection_list_template_draft(&section.title, parent.as_deref())
    } else if matches!(input.role, TemplateSemanticCreateRole::SectionElement)
        && input.include_page_content
    {
        collection_item_template_draft(parent.as_deref(), true)
    } else {
        template_draft(draft_role, parent.as_deref())
    };
    let mut mutations = vec![WorkspaceResourceMutation {
        relative_path: destination,
        contents,
        create_only: true,
    }];
    let receipt_path = if let Some(section) = new_section {
        let section_path = section.relative_path;
        mutations.push(WorkspaceResourceMutation {
            relative_path: section_path.clone(),
            contents: section.contents,
            create_only: true,
        });
        section_path
    } else if let Some((content_path, key)) = assignment {
        let source = workspace.documents.text_for(&content_path).ok_or_else(|| {
            format!(
                "ProjectWorkspace nu urmărește ținta semantică {content_path} pentru atribuire."
            )
        })?;
        let next = rewrite_frontmatter_template_assignment(&source, key, Some(&logical_name))?;
        if next != source {
            mutations.push(WorkspaceResourceMutation {
                relative_path: content_path.clone(),
                contents: next,
                create_only: false,
            });
        }
        content_path
    } else {
        mutations[0].relative_path.clone()
    };

    let creates_section = input.new_section.is_some();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata(
                if creates_section {
                    "Creare secțiune și șablon de arhivă Zola/Tera"
                } else {
                    "Creare șablon semantic Zola/Tera"
                },
                if creates_section {
                    "templates.create_archive_section"
                } else {
                    "templates.create_semantic"
                },
            ),
            mutations,
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_create_template_collection(
    input: CreateTemplateCollectionInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("Colecția are nevoie de un nume.".to_string());
    }
    let slug = collection_slug(if input.slug.trim().is_empty() {
        title
    } else {
        input.slug.as_str()
    });
    if slug.is_empty() {
        return Err("Colecția are nevoie de un slug valid.".to_string());
    }
    let list_path = local_template_path(&input.list_template_name)?;
    let item_path = local_template_path(&input.item_template_name)?;
    if list_path == item_path {
        return Err("Șablonul listei și cel al elementului trebuie să fie diferite.".to_string());
    }
    let section_path = format!("content/{slug}/_index.md");
    let list_name = template_logical_name(&list_path)?;
    let item_name = template_logical_name(&item_path)?;

    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    for path in [&list_path, &item_path, &section_path] {
        require_destination_available(workspace, path)?;
    }
    let parent = validate_parent_template(
        &root,
        workspace,
        input.parent_template_name.as_deref(),
        None,
    )?;
    let list_contents = collection_list_template_draft(title, parent.as_deref());
    let item_contents =
        collection_item_template_draft(parent.as_deref(), input.include_page_content);
    let section_contents = collection_section_frontmatter(title, &list_name, &item_name);
    let receipt_path = section_path.clone();

    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare colecție Tera/Zola", "templates.create_collection"),
            vec![
                WorkspaceResourceMutation {
                    relative_path: list_path,
                    contents: list_contents,
                    create_only: true,
                },
                WorkspaceResourceMutation {
                    relative_path: item_path,
                    contents: item_contents,
                    create_only: true,
                },
                WorkspaceResourceMutation {
                    relative_path: section_path,
                    contents: section_contents,
                    create_only: true,
                },
            ],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_duplicate_template(
    input: DuplicateTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_template_source_path(&input.source_relative_path)?;
    let destination = local_template_path(&input.destination_name)?;
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    require_destination_available(workspace, &destination)?;
    let contents = require_template_text(workspace, &source)?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Duplicare șablon Tera", "templates.duplicate"),
            vec![WorkspaceResourceMutation {
                relative_path: destination,
                contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_override_theme_template(
    input: OverrideThemeTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_template_source_path(&input.source_relative_path)?;
    theme_template_name(&source).ok_or_else(|| {
        "Suprascrierea locală cere un șablon provenit din tema activă.".to_string()
    })?;
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    let catalog = build_template_catalog(&graph);
    let entry = catalog
        .resources
        .iter()
        .find(|entry| entry.file == source && entry.effective && !entry.editable)
        .ok_or_else(|| {
            format!("Catalogul Rust nu confirmă {source} drept șablon efectiv al temei active.")
        })?;
    let destination = local_template_path(&entry.name)?;
    require_destination_available(workspace, &destination)?;
    let contents = require_template_text(workspace, &source)?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata(
                "Suprascriere locală șablon Tera",
                "templates.override_theme",
            ),
            vec![WorkspaceResourceMutation {
                relative_path: destination,
                contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_rename_template(
    input: RenameTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_template_source_path(&input.source_relative_path)?;
    if !source.starts_with("templates/") {
        return Err(
            "Șabloanele temei sunt read-only. Creează o suprascriere locală înainte de redenumire."
                .to_string(),
        );
    }
    let destination = local_template_path(&input.destination_name)?;
    if source == destination {
        return Err("Redenumirea nu schimbă numele șablonului.".to_string());
    }

    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        stage_template_rename(&root, candidate, source, destination, now_ms())
    })
}

#[tauri::command(async)]
pub fn workspace_set_template_parent(
    input: SetTemplateParentInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_template_source_path(&input.relative_path)?;
    if !source.starts_with("templates/") {
        return Err(
            "Layout-ul unui șablon din temă se schimbă numai după o suprascriere locală."
                .to_string(),
        );
    }
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    let source_text = require_template_text(workspace, &source)?;
    let source_name = template_logical_name(&source)?;
    let parent = validate_parent_template(
        &root,
        workspace,
        input.parent_template_name.as_deref(),
        Some(&source_name),
    )?;
    let next = rewrite_template_parent(&source_text, parent.as_deref())?;
    if next == source_text {
        return Err("Șablonul are deja acest layout părinte.".to_string());
    }
    let receipt_path = source.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Schimbare layout părinte Tera", "templates.set_parent"),
            vec![WorkspaceResourceMutation {
                relative_path: source,
                contents: next,
                create_only: false,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_set_template_assignment(
    input: SetTemplateAssignmentInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let content_path = normalize_project_relative_path(&input.content_relative_path)?;
    if !content_path.starts_with("content/") || !content_path.ends_with(".md") {
        return Err("Atribuirea cere un fișier Markdown din content/.".to_string());
    }
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    let source_text = workspace.documents.text_for(&content_path).ok_or_else(|| {
        format!("ProjectWorkspace nu urmărește pagina {content_path} pentru atribuire.")
    })?;
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    let page = graph
        .pages
        .iter()
        .find(|page| page.file == content_path)
        .ok_or_else(|| format!("Catalogul Rust nu conține pagina {content_path}."))?;
    if matches!(input.key, TemplateAssignmentKey::PageTemplate)
        && matches!(page.page_kind, SourcePageKind::Page)
    {
        return Err(
            "page_template poate fi atribuit numai unei secțiuni sau paginii de start.".to_string(),
        );
    }

    let template_name = input
        .template_name
        .as_deref()
        .map(normalize_zola_template_reference)
        .filter(|name| !name.is_empty());
    if let Some(template_name) = template_name.as_deref() {
        let catalog = build_template_catalog(&graph);
        let entry = catalog
            .resources
            .iter()
            .find(|entry| {
                entry.effective && normalize_zola_template_reference(&entry.name) == template_name
            })
            .ok_or_else(|| {
                format!("Catalogul Rust nu găsește șablonul efectiv {template_name}.")
            })?;
        if entry.roles.iter().all(|role| {
            !matches!(
                role,
                crate::source_graph::template_catalog::TemplateCatalogRole::Page
            )
        }) {
            return Err(
                "Atribuirea acceptă numai șabloane de pagină/secțiune, nu layout-uri sau resurse reutilizabile."
                    .to_string(),
            );
        }
    }

    let next =
        rewrite_frontmatter_template_assignment(&source_text, input.key, template_name.as_deref())?;
    if next == source_text {
        return Err("Pagina are deja această atribuire.".to_string());
    }
    let receipt_path = content_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata(
                "Schimbare atribuire șablon Zola",
                "templates.set_assignment",
            ),
            vec![WorkspaceResourceMutation {
                relative_path: content_path,
                contents: next,
                create_only: false,
            }],
            now_ms(),
        )
    })
}

fn stage_template_rename(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    source: String,
    destination: String,
    changed_at_ms: u128,
) -> Result<ProjectWorkspaceMutationReceipt, String> {
    require_destination_available(workspace, &destination)?;
    let contents = require_template_text(workspace, &source)?;
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(project_root, &projection)?;
    let rewrite = plan_template_reference_workspace_mutation_from_graph(
        project_root,
        &workspace.documents,
        &graph,
        SourceGraphRewriteOperation::Rename,
        &source,
        &destination,
    )?;
    let mut mutations = vec![WorkspaceResourceMutation {
        relative_path: destination.clone(),
        contents,
        create_only: true,
    }];
    if let Some(reference_mutation) = rewrite.workspace_mutation {
        mutations.extend(
            reference_mutation
                .changes
                .into_iter()
                .filter(|change| change.relative_path != source)
                .map(|change| WorkspaceResourceMutation {
                    relative_path: change.relative_path,
                    contents: change.new_text,
                    create_only: false,
                }),
        );
    }

    workspace.stage_composite_changes(
        &current_workspace_identity(workspace),
        mutation_metadata("Redenumire șablon Tera și referințe", "templates.rename"),
        mutations,
        vec![WorkspaceResourceDelete {
            relative_path: source,
        }],
        None,
        changed_at_ms,
    )
}

#[tauri::command(async)]
pub fn workspace_delete_template(
    input: DeleteTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let relative_path = normalize_template_source_path(&input.relative_path)?;
    if !relative_path.starts_with("templates/") {
        return Err("Șabloanele temei nu pot fi șterse din proiect.".to_string());
    }
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    require_template_text(workspace, &relative_path)?;
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    let catalog = build_template_catalog(&graph);
    let entry = catalog
        .resources
        .iter()
        .find(|entry| entry.file == relative_path)
        .ok_or_else(|| format!("Catalogul Rust nu conține șablonul {relative_path}."))?;
    if let Some(diagnostic) = entry.delete_blocked_diagnostic.as_ref() {
        return Err(serde_json::to_string(diagnostic).unwrap_or_else(|error| {
            format!("Could not serialize the template deletion diagnostic: {error}")
        }));
    }

    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_changes(
            &current_workspace_identity(candidate),
            mutation_metadata("Ștergere șablon Tera", "templates.delete"),
            Vec::new(),
            vec![WorkspaceResourceDelete { relative_path }],
            now_ms(),
        )
    })
}

fn live_workspace<'a>(
    slot: &'a mut std::sync::MutexGuard<'_, Option<ProjectWorkspace>>,
) -> Result<&'a mut ProjectWorkspace, String> {
    slot.as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())
}

fn semantic_creation_assignment(
    workspace: &ProjectWorkspace,
    graph: &crate::source_graph::SourceGraph,
    role: TemplateSemanticCreateRole,
    target_id: Option<&str>,
    logical_name: &str,
) -> Result<Option<(String, TemplateAssignmentKey)>, String> {
    match role {
        TemplateSemanticCreateRole::Layout | TemplateSemanticCreateRole::Custom => {
            if target_id.is_some() {
                return Err("Acest rol creează o resursă fără atribuire Zola.".to_string());
            }
            Ok(None)
        }
        TemplateSemanticCreateRole::DefaultPage => {
            if target_id.is_some() {
                return Err(
                    "Rolul implicit page.html este o convenție de site și nu acceptă o țintă individuală."
                        .to_string(),
                );
            }
            if logical_name != "page.html" {
                return Err(
                    "Șablonul implicit al paginilor trebuie creat cu numele page.html.".to_string(),
                );
            }
            Ok(None)
        }
        TemplateSemanticCreateRole::NotFound => {
            if target_id.is_some() {
                return Err(
                    "Rolul 404 este o convenție de sistem și nu acceptă o țintă individuală."
                        .to_string(),
                );
            }
            if logical_name != "404.html" {
                return Err("Pagina de sistem 404 trebuie creată ca 404.html.".to_string());
            }
            Ok(None)
        }
        TemplateSemanticCreateRole::Homepage => {
            let target_id = target_id.ok_or_else(|| {
                "Crearea șablonului paginii principale cere ținta Home exactă.".to_string()
            })?;
            let page = graph
                .pages
                .iter()
                .find(|page| {
                    matches!(page.page_kind, SourcePageKind::Home)
                        && target_matches_page(Some(target_id), page)
                })
                .ok_or_else(|| {
                    "Catalogul Rust nu găsește pagina principală indicată.".to_string()
                })?;
            Ok(Some((page.file.clone(), TemplateAssignmentKey::Template)))
        }
        TemplateSemanticCreateRole::SpecificPage => {
            let target_id = target_id.ok_or_else(|| {
                "Crearea unui șablon de pagină cere pagina țintă exactă.".to_string()
            })?;
            let page = graph
                .pages
                .iter()
                .find(|page| {
                    matches!(page.page_kind, SourcePageKind::Page)
                        && target_matches_page(Some(target_id), page)
                })
                .ok_or_else(|| format!("Catalogul Rust nu găsește pagina țintă {target_id}."))?;
            Ok(Some((page.file.clone(), TemplateAssignmentKey::Template)))
        }
        TemplateSemanticCreateRole::SectionArchive | TemplateSemanticCreateRole::SectionElement => {
            let target_id = target_id.ok_or_else(|| {
                "Crearea șablonului de secțiune cere secțiunea țintă exactă.".to_string()
            })?;
            let section = graph
                .pages
                .iter()
                .find(|page| {
                    matches!(page.page_kind, SourcePageKind::Section)
                        && target_matches_page(Some(target_id), page)
                })
                .ok_or_else(|| format!("Catalogul Rust nu găsește secțiunea țintă {target_id}."))?;
            let key = if matches!(role, TemplateSemanticCreateRole::SectionArchive) {
                TemplateAssignmentKey::Template
            } else {
                TemplateAssignmentKey::PageTemplate
            };
            Ok(Some((section.file.clone(), key)))
        }
        TemplateSemanticCreateRole::TaxonomyList | TemplateSemanticCreateRole::TaxonomyTerm => {
            let target_id = target_id.ok_or_else(|| {
                "Crearea șablonului de taxonomie cere taxonomia țintă exactă.".to_string()
            })?;
            let (config_path, config_source) = ["zola.toml", "config.toml"]
                .iter()
                .find_map(|path| {
                    workspace
                        .documents
                        .text_for(path)
                        .map(|source| ((*path).to_string(), source))
                })
                .ok_or_else(|| {
                    "Catalogul semantic cere zola.toml sau config.toml în ProjectWorkspace."
                        .to_string()
                })?;
            let catalog = build_taxonomy_catalog(graph, &config_path, &config_source);
            let taxonomy = catalog
                .entries
                .iter()
                .find(|entry| {
                    entry.declared
                        && entry.render
                        && (entry.id == target_id || entry.name == target_id)
                })
                .ok_or_else(|| {
                    format!(
                        "Catalogul Rust nu găsește o taxonomie declarată și randată pentru ținta {target_id}."
                    )
                })?;
            let expected_name = if matches!(role, TemplateSemanticCreateRole::TaxonomyList) {
                format!("{}/list.html", taxonomy.name)
            } else {
                format!("{}/single.html", taxonomy.name)
            };
            if normalize_zola_template_reference(logical_name) != expected_name {
                return Err(format!(
                    "Rolul selectat este rezolvat de Zola numai prin convenția {expected_name}."
                ));
            }
            Ok(None)
        }
    }
}

fn target_matches_page(
    target_id: Option<&str>,
    page: &crate::source_graph::model::SourceGraphPage,
) -> bool {
    target_id.is_none_or(|target| target == page.id || target == page.file || target == page.url)
}

fn local_template_path(name: &str) -> Result<String, String> {
    let logical = name
        .trim()
        .replace('\\', "/")
        .trim_start_matches("templates/")
        .to_string();
    if logical.is_empty() {
        return Err("Numele șablonului este obligatoriu.".to_string());
    }
    let logical = if logical.ends_with(".html") {
        logical
    } else {
        format!("{logical}.html")
    };
    let path = normalize_project_relative_path(&format!("templates/{logical}"))?;
    if !path.starts_with("templates/") || !path.ends_with(".html") {
        return Err("Șablonul trebuie să fie un fișier .html din templates/.".to_string());
    }
    Ok(path)
}

fn normalize_template_source_path(path: &str) -> Result<String, String> {
    let path = normalize_project_relative_path(path)?;
    let is_local = path.starts_with("templates/");
    let is_theme = theme_template_name(&path).is_some();
    if (!is_local && !is_theme) || !path.ends_with(".html") {
        return Err(
            "Operația este permisă numai pentru fișiere .html din templates/ sau themes/*/templates/."
                .to_string(),
        );
    }
    Ok(path)
}

fn theme_template_name(path: &str) -> Option<&str> {
    let remainder = path.strip_prefix("themes/")?;
    let (_theme, remainder) = remainder.split_once('/')?;
    remainder.strip_prefix("templates/")
}

fn require_template_text(
    workspace: &ProjectWorkspace,
    relative_path: &str,
) -> Result<String, String> {
    workspace
        .documents
        .text_for(relative_path)
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește textul șablonului {relative_path}."))
}

fn require_destination_available(
    workspace: &ProjectWorkspace,
    relative_path: &str,
) -> Result<(), String> {
    if workspace.documents.files.contains_key(relative_path) {
        return Err(format!("Șablonul {relative_path} există deja în sesiune."));
    }
    Ok(())
}

fn template_logical_name(relative_path: &str) -> Result<String, String> {
    relative_path
        .strip_prefix("templates/")
        .map(normalize_zola_template_reference)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("Calea {relative_path} nu este un șablon local valid."))
}

fn validate_parent_template(
    project_root: &Path,
    workspace: &ProjectWorkspace,
    parent_template_name: Option<&str>,
    source_template_name: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(parent_name) = parent_template_name
        .map(normalize_zola_template_reference)
        .filter(|name| !name.is_empty())
    else {
        return Ok(None);
    };
    if source_template_name
        .is_some_and(|source| normalize_zola_template_reference(source) == parent_name)
    {
        return Err("Un șablon nu se poate extinde pe sine.".to_string());
    }

    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(project_root, &projection)?;
    let catalog = build_template_catalog(&graph);
    let parent = catalog
        .resources
        .iter()
        .find(|entry| {
            entry.effective && normalize_zola_template_reference(&entry.name) == parent_name
        })
        .ok_or_else(|| format!("Catalogul Rust nu găsește layout-ul efectiv {parent_name}."))?;
    if !parent
        .roles
        .contains(&crate::source_graph::template_catalog::TemplateCatalogRole::Layout)
    {
        return Err(format!(
            "{parent_name} nu este detectat drept layout Tera și nu poate deveni Master."
        ));
    }

    if let Some(source_name) = source_template_name {
        let source_name = normalize_zola_template_reference(source_name);
        let effective_by_name = catalog
            .resources
            .iter()
            .filter(|entry| entry.effective)
            .map(|entry| {
                (
                    normalize_zola_template_reference(&entry.name),
                    entry
                        .extends
                        .as_deref()
                        .map(normalize_zola_template_reference),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let mut current = Some(parent_name.clone());
        let mut visited = std::collections::HashSet::new();
        while let Some(name) = current {
            if name == source_name {
                return Err("Schimbarea layout-ului ar crea un ciclu Tera extends.".to_string());
            }
            if !visited.insert(name.clone()) {
                return Err("Lanțul layout-urilor conține deja un ciclu Tera.".to_string());
            }
            current = effective_by_name.get(&name).cloned().flatten();
        }
    }

    Ok(Some(parent_name))
}

fn rewrite_template_parent(source: &str, parent: Option<&str>) -> Result<String, String> {
    let extends = parse_tera_items(source)
        .into_iter()
        .filter(|item| {
            item.kind == TeraItemKind::Node
                && item.node_kind.as_ref() == Some(&SourceNodeKind::Extends)
        })
        .collect::<Vec<_>>();
    if extends.len() > 1 {
        return Err(
            "Șablonul conține mai multe directive extends; repară sursa înainte de schimbarea layout-ului."
                .to_string(),
        );
    }

    match (extends.first(), parent) {
        (Some(item), Some(parent)) => {
            let replacement = format!("{{% extends \"{}\" %}}", escape_tera_string(parent));
            if source.get(item.start..item.end) == Some(replacement.as_str()) {
                return Ok(source.to_string());
            }
            let mut next = source.to_string();
            next.replace_range(item.start..item.end, &replacement);
            Ok(next)
        }
        (Some(item), None) => {
            let (start, end) = removable_source_range(source, item.start, item.end);
            let mut next = source.to_string();
            next.replace_range(start..end, "");
            Ok(next)
        }
        (None, Some(parent)) => Ok(format!(
            "{{% extends \"{}\" %}}\n\n{}",
            escape_tera_string(parent),
            source.trim_start()
        )),
        (None, None) => Ok(source.to_string()),
    }
}

fn removable_source_range(source: &str, start: usize, end: usize) -> (usize, usize) {
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_break = source[end..].find('\n').map(|offset| end + offset);
    let line_end = line_break.unwrap_or(source.len());
    if source[line_start..start].trim().is_empty() && source[end..line_end].trim().is_empty() {
        let mut removable_end = line_break.map_or(line_end, |index| index + 1);
        if source[removable_end..].starts_with('\n') {
            removable_end += 1;
        }
        (line_start, removable_end)
    } else {
        (start, end)
    }
}

fn rewrite_frontmatter_template_assignment(
    source: &str,
    key: TemplateAssignmentKey,
    template_name: Option<&str>,
) -> Result<String, String> {
    let (start, end) = zola_frontmatter_range(source)
        .ok_or_else(|| "Pagina nu are frontmatter Zola delimitat valid.".to_string())?;
    let frontmatter = &source[start..end];
    let key = match key {
        TemplateAssignmentKey::Template => "template",
        TemplateAssignmentKey::PageTemplate => "page_template",
    };
    let is_toml = source.trim_start_matches('\u{feff}').starts_with("+++");
    let rendered = if is_toml {
        let mut document = frontmatter
            .parse::<DocumentMut>()
            .map_err(|error| format!("Frontmatter TOML invalid: {error}"))?;
        if let Some(template_name) = template_name {
            document[key] = value(template_name);
        } else {
            document.as_table_mut().remove(key);
        }
        document.to_string()
    } else {
        let mut document = serde_yaml::from_str::<serde_yaml::Value>(frontmatter)
            .map_err(|error| format!("Frontmatter YAML invalid: {error}"))?;
        let mapping = document
            .as_mapping_mut()
            .ok_or_else(|| "Frontmatter YAML trebuie să fie un obiect.".to_string())?;
        let yaml_key = serde_yaml::Value::String(key.to_string());
        if let Some(template_name) = template_name {
            mapping.insert(
                yaml_key,
                serde_yaml::Value::String(template_name.to_string()),
            );
        } else {
            mapping.remove(&yaml_key);
        }
        serde_yaml::to_string(&document)
            .map(|rendered| rendered.trim_start_matches("---\n").to_string())
            .map_err(|error| format!("Frontmatter YAML nu poate fi serializat: {error}"))?
    };
    let rendered = if rendered.starts_with('\n') {
        rendered
    } else {
        format!("\n{}", rendered.trim_end())
    };
    let mut next = source.to_string();
    next.replace_range(start..end, &rendered);
    Ok(next)
}

fn template_draft(role: TemplateDraftRole, parent: Option<&str>) -> String {
    match role {
        TemplateDraftRole::Page => format!(
            "{}{{% block content %}}\n<main>\n  <h1>Șablon nou</h1>\n</main>\n{{% endblock content %}}\n",
            extends_prefix(parent)
        ),
        TemplateDraftRole::Layout if parent.is_some() => format!(
            "{}{{% block content %}}\n  {{% block page_content %}}{{% endblock page_content %}}\n{{% endblock content %}}\n",
            extends_prefix(parent)
        ),
        TemplateDraftRole::Layout => "<!doctype html>\n<html lang=\"ro\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>{% block title %}{{ config.title }}{% endblock title %}</title>\n</head>\n<body>\n  {% block content %}{% endblock content %}\n</body>\n</html>\n".to_string(),
        TemplateDraftRole::Partial => "<div>\n  Fragment nou\n</div>\n".to_string(),
        TemplateDraftRole::MacroLibrary => {
            "{% macro exemplu(text) %}\n  <span>{{ text }}</span>\n{% endmacro exemplu %}\n"
                .to_string()
        }
    }
}

fn collection_list_template_draft(title: &str, parent: Option<&str>) -> String {
    format!(
        "{}{{% block content %}}\n<section class=\"colectie\">\n  <header class=\"colectie-header\">\n    <h1>{{{{ section.title | default(value=\"{}\") }}}}</h1>\n    {{% if section.description %}}<p>{{{{ section.description }}}}</p>{{% endif %}}\n  </header>\n  <div class=\"colectie-lista\">\n    {{% for entry in paginator.pages %}}\n      <article class=\"colectie-card\">\n        <h2><a href=\"{{{{ entry.permalink }}}}\">{{{{ entry.title }}}}</a></h2>\n        {{% if entry.description %}}<p>{{{{ entry.description }}}}</p>{{% endif %}}\n      </article>\n    {{% endfor %}}\n  </div>\n  {{% if paginator.number_pagers > 1 %}}\n    <nav class=\"paginare\" aria-label=\"Paginare\">\n      {{% if paginator.previous %}}<a href=\"{{{{ paginator.previous }}}}\">Pagina anterioară</a>{{% endif %}}\n      <span>Pagina {{{{ paginator.current_index }}}} din {{{{ paginator.number_pagers }}}}</span>\n      {{% if paginator.next %}}<a href=\"{{{{ paginator.next }}}}\">Pagina următoare</a>{{% endif %}}\n    </nav>\n  {{% endif %}}\n</section>\n{{% endblock content %}}\n",
        extends_prefix(parent),
        escape_tera_string(title),
    )
}

fn collection_item_template_draft(parent: Option<&str>, include_page_content: bool) -> String {
    let content = if include_page_content {
        "  <div class=\"articol-continut\">\n    {{ page.content | safe }}\n  </div>\n"
    } else {
        ""
    };
    format!(
        "{}{{% block content %}}\n<article class=\"articol\">\n  <header class=\"articol-header\">\n    <h1>{{{{ page.title }}}}</h1>\n    {{% if page.date %}}<time datetime=\"{{{{ page.date }}}}\">{{{{ page.date }}}}</time>{{% endif %}}\n  </header>\n{content}</article>\n{{% endblock content %}}\n",
        extends_prefix(parent),
    )
}

fn collection_section_frontmatter(
    title: &str,
    list_template_name: &str,
    item_template_name: &str,
) -> String {
    format!(
        "+++\ntitle = \"{}\"\ntemplate = \"{}\"\npage_template = \"{}\"\nsort_by = \"date\"\npaginate_by = {}\npaginate_path = \"{}\"\n+++\n",
        escape_toml_string(title),
        escape_toml_string(list_template_name),
        escape_toml_string(item_template_name),
        DEFAULT_ARCHIVE_PAGINATE_BY,
        DEFAULT_ARCHIVE_PAGINATE_PATH,
    )
}

fn prepare_semantic_section(
    input: &CreateSemanticSectionInput,
    list_template_name: &str,
) -> Result<PreparedSemanticSection, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("Secțiunea nouă are nevoie de un nume.".to_string());
    }
    let slug = collection_slug(if input.slug.trim().is_empty() {
        title
    } else {
        input.slug.as_str()
    });
    if slug.is_empty() {
        return Err("Secțiunea nouă are nevoie de un slug valid.".to_string());
    }
    let sort_by = input
        .sort_by
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("weight");
    if !matches!(sort_by, "none" | "date" | "title" | "weight") {
        return Err(format!(
            "Ordinea secțiunii {sort_by} nu este acceptată de creatorul de arhive."
        ));
    }

    Ok(PreparedSemanticSection {
        title: title.to_string(),
        relative_path: format!("content/{slug}/_index.md"),
        contents: semantic_section_frontmatter(title, list_template_name, sort_by),
    })
}

fn semantic_section_frontmatter(title: &str, template_name: &str, sort_by: &str) -> String {
    format!(
        "+++\ntitle = \"{}\"\ntemplate = \"{}\"\nsort_by = \"{}\"\npaginate_by = {}\npaginate_path = \"{}\"\n+++\n",
        escape_toml_string(title),
        escape_toml_string(template_name),
        escape_toml_string(sort_by),
        DEFAULT_ARCHIVE_PAGINATE_BY,
        DEFAULT_ARCHIVE_PAGINATE_PATH,
    )
}

fn extends_prefix(parent: Option<&str>) -> String {
    parent
        .map(|parent| format!("{{% extends \"{}\" %}}\n\n", escape_tera_string(parent)))
        .unwrap_or_default()
}

fn collection_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_dash = false;
    for character in value.trim().chars() {
        let normalized = match character {
            'ă' | 'â' | 'Ă' | 'Â' => Some('a'),
            'î' | 'Î' => Some('i'),
            'ș' | 'ş' | 'Ș' | 'Ş' => Some('s'),
            'ț' | 'ţ' | 'Ț' | 'Ţ' => Some('t'),
            character if character.is_ascii_alphanumeric() => Some(character.to_ascii_lowercase()),
            _ => None,
        };
        if let Some(character) = normalized {
            slug.push(character);
            previous_was_dash = false;
        } else if !previous_was_dash {
            slug.push('-');
            previous_was_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_tera_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::{ProjectWorkspaceIdentity, WorkspaceHistoryDirection},
        },
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest, ProjectDiskManifestEntry},
    };

    use super::*;

    #[test]
    fn local_template_path_is_canonical_and_adds_html_extension() {
        assert_eq!(
            local_template_path("partials/card").unwrap(),
            "templates/partials/card.html"
        );
        assert_eq!(
            local_template_path("templates/page.html").unwrap(),
            "templates/page.html"
        );
        assert!(local_template_path("../outside.html").is_err());
    }

    #[test]
    fn theme_template_name_extracts_only_theme_template_paths() {
        assert_eq!(
            theme_template_name("themes/pana/templates/base.html"),
            Some("base.html")
        );
        assert_eq!(theme_template_name("templates/base.html"), None);
    }

    #[test]
    fn parent_rewrite_preserves_the_template_body_and_supports_removal() {
        let source =
            "{% extends \"base.html\" %}\n\n{% block content %}Salut{% endblock content %}\n";
        let changed = rewrite_template_parent(source, Some("layout.html")).unwrap();
        assert!(changed.starts_with("{% extends \"layout.html\" %}"));
        assert!(changed.contains("{% block content %}Salut{% endblock content %}"));

        let removed = rewrite_template_parent(&changed, None).unwrap();
        assert!(!removed.contains("{% extends"));
        assert!(removed.starts_with("{% block content %}"));

        let added = rewrite_template_parent(
            "{% block content %}Salut{% endblock content %}\n",
            Some("base.html"),
        )
        .unwrap();
        assert!(added.starts_with("{% extends \"base.html\" %}\n\n"));
    }

    #[test]
    fn assignment_rewrite_supports_toml_yaml_and_clearing() {
        let toml = "+++\ntitle = \"Blog\"\n+++\n";
        let assigned = rewrite_frontmatter_template_assignment(
            toml,
            TemplateAssignmentKey::PageTemplate,
            Some("blog/single.html"),
        )
        .unwrap();
        assert!(assigned.contains("page_template = \"blog/single.html\""));
        let cleared = rewrite_frontmatter_template_assignment(
            &assigned,
            TemplateAssignmentKey::PageTemplate,
            None,
        )
        .unwrap();
        assert!(!cleared.contains("page_template"));

        let yaml = "---\ntitle: Blog\n---\n";
        let assigned = rewrite_frontmatter_template_assignment(
            yaml,
            TemplateAssignmentKey::Template,
            Some("blog/list.html"),
        )
        .unwrap();
        assert!(assigned.contains("template: blog/list.html"));
        assert!(assigned.contains("title: Blog"));
    }

    #[test]
    fn collection_drafts_keep_html_content_optional_and_assign_both_templates() {
        let section =
            collection_section_frontmatter("Noutăți", "noutati/list.html", "noutati/single.html");
        assert!(section.contains("template = \"noutati/list.html\""));
        assert!(section.contains("page_template = \"noutati/single.html\""));
        assert!(section.contains("sort_by = \"date\""));
        assert!(section.contains("paginate_by = 6"));
        assert!(section.contains("paginate_path = \"pagina\""));

        let archive = collection_list_template_draft("Noutăți", Some("layout.html"));
        assert!(archive.starts_with("{% extends \"layout.html\" %}"));
        assert!(archive.contains("paginator.pages"));
        assert!(archive.contains("paginator.number_pagers > 1"));
        assert!(!archive.contains("section.pages"));

        let visual = collection_item_template_draft(Some("layout.html"), false);
        assert!(visual.starts_with("{% extends \"layout.html\" %}"));
        assert!(!visual.contains("page.content"));
        let content_driven = collection_item_template_draft(Some("layout.html"), true);
        assert!(content_driven.contains("page.content | safe"));
    }

    #[test]
    fn semantic_archive_prepares_its_new_section_and_assignment_atomically() {
        let section = prepare_semantic_section(
            &CreateSemanticSectionInput {
                title: "Noutăți și Servicii".to_string(),
                slug: "Noutăți și Servicii".to_string(),
                sort_by: Some("weight".to_string()),
            },
            "noutati-servicii/arhiva.html",
        )
        .unwrap();

        assert_eq!(
            section.relative_path,
            "content/noutati-si-servicii/_index.md"
        );
        assert!(section.contents.contains("title = \"Noutăți și Servicii\""));
        assert!(section
            .contents
            .contains("template = \"noutati-servicii/arhiva.html\""));
        assert!(section.contents.contains("sort_by = \"weight\""));
        assert!(!section.contents.contains("page_template"));
    }

    #[test]
    fn semantic_archive_rejects_an_unknown_section_order() {
        let error = prepare_semantic_section(
            &CreateSemanticSectionInput {
                title: "Servicii".to_string(),
                slug: "servicii".to_string(),
                sort_by: Some("aleator".to_string()),
            },
            "servicii/arhiva.html",
        )
        .unwrap_err();

        assert!(error.contains("aleator"));
    }

    #[test]
    fn semantic_creation_validates_exact_targets_and_independent_section_keys() {
        let root = std::env::temp_dir().join(format!(
            "pana-template-semantic-create-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let config = "base_url = \"https://example.test\"\ntaxonomies = [{ name = \"tags\" }]\n";
        let home = "+++\ntitle = \"Acasă\"\n+++\n";
        let section = "+++\ntitle = \"Blog\"\n+++\n";
        let workspace = test_workspace_with_text_files(
            &root,
            &[
                (
                    "zola.toml",
                    config,
                    TextBufferLanguage::Toml,
                    TextBufferRole::Config,
                ),
                (
                    "content/_index.md",
                    home,
                    TextBufferLanguage::Markdown,
                    TextBufferRole::Page,
                ),
                (
                    "content/blog/_index.md",
                    section,
                    TextBufferLanguage::Markdown,
                    TextBufferRole::Page,
                ),
            ],
        );
        let projection = workspace.capture_projection_snapshot().unwrap();
        let graph = build_source_graph_from_workspace_projection(&root, &projection).unwrap();

        let archive = semantic_creation_assignment(
            &workspace,
            &graph,
            TemplateSemanticCreateRole::SectionArchive,
            Some("content/blog/_index.md"),
            "blog/list.html",
        )
        .unwrap();
        assert_eq!(
            archive,
            Some((
                "content/blog/_index.md".to_string(),
                TemplateAssignmentKey::Template
            ))
        );

        let element = semantic_creation_assignment(
            &workspace,
            &graph,
            TemplateSemanticCreateRole::SectionElement,
            Some("content/blog/_index.md"),
            "blog/single.html",
        )
        .unwrap();
        assert_eq!(
            element,
            Some((
                "content/blog/_index.md".to_string(),
                TemplateAssignmentKey::PageTemplate
            ))
        );
        assert!(semantic_creation_assignment(
            &workspace,
            &graph,
            TemplateSemanticCreateRole::Homepage,
            None,
            "index.html",
        )
        .is_err());
        assert!(semantic_creation_assignment(
            &workspace,
            &graph,
            TemplateSemanticCreateRole::DefaultPage,
            None,
            "not-page.html",
        )
        .is_err());
        assert!(semantic_creation_assignment(
            &workspace,
            &graph,
            TemplateSemanticCreateRole::TaxonomyList,
            Some("tags"),
            "tags/list.html",
        )
        .is_ok());
        assert!(semantic_creation_assignment(
            &workspace,
            &graph,
            TemplateSemanticCreateRole::TaxonomyTerm,
            Some("tags"),
            "tags/list.html",
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rename_is_one_atomic_history_entry_and_round_trips_through_undo_redo() {
        let root = std::env::temp_dir().join(format!(
            "pana-template-rename-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        let config = "base_url = \"https://example.test\"\n";
        fs::write(root.join("zola.toml"), config).unwrap();
        let base = r#"{% include "partials/header.html" %}"#;
        let header = "<header>Header</header>";
        fs::write(root.join("templates/base.html"), base).unwrap();
        fs::write(root.join("templates/partials/header.html"), header).unwrap();

        let session = test_session(&root);
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 32,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        insert_text(
            &mut documents,
            &root,
            "zola.toml",
            config,
            TextBufferLanguage::Toml,
            TextBufferRole::Config,
        );
        insert_template(&mut documents, &root, "templates/base.html", base);
        insert_template(
            &mut documents,
            &root,
            "templates/partials/header.html",
            header,
        );
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            ProjectDiskManifest {
                root: session.project_root.clone(),
                files: vec![
                    ProjectDiskManifestEntry {
                        relative_path: "templates/base.html".to_string(),
                        modified_ms: 1,
                        size: base.len() as u64,
                        version_token: String::new(),
                    },
                    ProjectDiskManifestEntry {
                        relative_path: "templates/partials/header.html".to_string(),
                        modified_ms: 1,
                        size: header.len() as u64,
                        version_token: String::new(),
                    },
                    ProjectDiskManifestEntry {
                        relative_path: "zola.toml".to_string(),
                        modified_ms: 1,
                        size: config.len() as u64,
                        version_token: String::new(),
                    },
                ],
                truncated: false,
                max_files: 100,
            },
        )
        .unwrap();
        let page_js = PageJsDraftStore::new(&session);
        let mut workspace = ProjectWorkspace::new(session, accepted, documents, page_js).unwrap();

        let receipt = stage_template_rename(
            &root,
            &mut workspace,
            "templates/partials/header.html".to_string(),
            "templates/partials/site-header.html".to_string(),
            2,
        )
        .unwrap();
        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(receipt.history.redo_count, 0);
        assert!(workspace
            .documents
            .text_for("templates/partials/header.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/site-header.html")
                .as_deref(),
            Some(header)
        );
        assert!(workspace
            .documents
            .text_for("templates/base.html")
            .unwrap()
            .contains("partials/site-header.html"));

        let undo = workspace.undo(&workspace_identity(&workspace), 3).unwrap();
        assert!(matches!(undo.direction, WorkspaceHistoryDirection::Undo));
        assert_eq!(undo.history.undo_count, 0);
        assert_eq!(undo.history.redo_count, 1);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/header.html")
                .as_deref(),
            Some(header)
        );
        assert!(workspace
            .documents
            .text_for("templates/partials/site-header.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/base.html")
                .as_deref(),
            Some(base)
        );

        let redo = workspace.redo(&workspace_identity(&workspace), 4).unwrap();
        assert!(matches!(redo.direction, WorkspaceHistoryDirection::Redo));
        assert_eq!(redo.history.undo_count, 1);
        assert_eq!(redo.history.redo_count, 0);
        assert!(workspace
            .documents
            .text_for("templates/partials/header.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/site-header.html")
                .as_deref(),
            Some(header)
        );
        assert!(workspace
            .documents
            .text_for("templates/base.html")
            .unwrap()
            .contains("partials/site-header.html"));

        fs::remove_dir_all(root).unwrap();
    }

    fn test_session(root: &Path) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "templates-operation-test".to_string(),
            project_root: root.to_string_lossy().to_string(),
            zola_root: root.to_string_lossy().to_string(),
            session_dir: root.join("session").to_string_lossy().to_string(),
            manifest_path: root.join("session.json").to_string_lossy().to_string(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root.to_string_lossy().to_string(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 2,
                directory_count: 3,
            },
        }
    }

    fn test_workspace_with_text_files(
        root: &Path,
        files: &[(&str, &str, TextBufferLanguage, TextBufferRole)],
    ) -> ProjectWorkspace {
        fs::create_dir_all(root).unwrap();
        for (relative_path, text, _, _) in files {
            let absolute_path = root.join(relative_path);
            if let Some(parent) = absolute_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(absolute_path, text).unwrap();
        }

        let session = test_session(root);
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 32,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        for (relative_path, text, language, role) in files {
            insert_text(&mut documents, root, relative_path, text, *language, *role);
        }
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            ProjectDiskManifest {
                root: session.project_root.clone(),
                files: files
                    .iter()
                    .map(|(relative_path, text, _, _)| ProjectDiskManifestEntry {
                        relative_path: (*relative_path).to_string(),
                        modified_ms: 1,
                        size: text.len() as u64,
                        version_token: String::new(),
                    })
                    .collect(),
                truncated: false,
                max_files: 100,
            },
        )
        .unwrap();
        let page_js = PageJsDraftStore::new(&session);
        ProjectWorkspace::new(session, accepted, documents, page_js).unwrap()
    }

    fn insert_template(store: &mut FileBufferStore, root: &Path, relative_path: &str, text: &str) {
        insert_text(
            store,
            root,
            relative_path,
            text,
            TextBufferLanguage::Html,
            TextBufferRole::Template,
        );
    }

    fn insert_text(
        store: &mut FileBufferStore,
        root: &Path,
        relative_path: &str,
        text: &str,
        language: TextBufferLanguage,
        role: TextBufferRole,
    ) {
        store.insert_loaded_file(FileBufferEntry {
            relative_path: relative_path.to_string(),
            absolute_path: root.join(relative_path).to_string_lossy().to_string(),
            language,
            role,
            baseline: FileBufferBaseline {
                hash: hash_text(text),
                modified_ms: 1,
                size: text.len() as u64,
                readonly: false,
            },
            baseline_text: text.to_string(),
            draft: None,
            revision: 1,
        });
    }

    fn workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }
}

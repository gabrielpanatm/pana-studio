use std::collections::{BTreeMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{
    project_model::{
        attribute_engine::raw_tag_attributes,
        model::ProjectModel,
        move_engine::{parse_html_tag_at, ProjectMovePosition},
    },
    source_graph::model::{SourceNode, SourceNodeKind},
};

use super::native_block_by_id;

pub const SLIDER_MIN_SLIDES: usize = 1;
pub const SLIDER_MAX_SLIDES: usize = 32;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockSlotMutationContext {
    pub provider_id: String,
    pub slot_id: String,
    pub root_source_id: String,
    pub expected_model_revision: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockSlotItemState {
    pub source_node_id: String,
    pub tag: String,
    pub label: String,
    pub index: usize,
    pub editable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockSlotState {
    pub id: String,
    pub item_kind: String,
    pub container_source_node_id: Option<String>,
    pub minimum_items: usize,
    pub maximum_items: Option<usize>,
    pub editable: bool,
    pub diagnostic: Option<String>,
    pub items: Vec<NativeBlockSlotItemState>,
}

pub(crate) fn inspect_native_block_slots(
    model: &ProjectModel,
    root: &SourceNode,
    provider_id: &str,
) -> Vec<NativeBlockSlotState> {
    let Some(definition) = native_block_by_id(provider_id) else {
        return Vec::new();
    };
    if !definition.capabilities.supports_slots {
        return Vec::new();
    }
    definition
        .slots
        .iter()
        .map(|slot| {
            if provider_id == "slider" && slot.id == "slides" {
                inspect_slider_slides(model, root)
            } else {
                NativeBlockSlotState {
                    id: slot.id.to_string(),
                    item_kind: slot.item_kind.to_string(),
                    container_source_node_id: None,
                    minimum_items: slot.minimum_items,
                    maximum_items: slot.maximum_items,
                    editable: false,
                    diagnostic: Some(
                        "Slotul este declarat în registry, dar nu are încă un inspector structural Rust."
                            .to_string(),
                    ),
                    items: Vec::new(),
                }
            }
        })
        .collect()
}

pub(crate) fn render_native_block_slot_item_html(
    provider_id: &str,
    slot_id: &str,
) -> Result<String, String> {
    match (provider_id.trim(), slot_id.trim()) {
        ("slider", "slides") => Ok(
            r#"<div class="slider__slide" data-pana-slider-slide role="group" aria-roledescription="slide">
  <h3>Slide nou</h3>
  <p>Editeaza continutul direct in Canvas.</p>
</div>"#
                .to_string(),
        ),
        (provider, slot) => Err(format!(
            "NativeBlockRegistry nu definește renderer de item pentru `{provider}:{slot}`."
        )),
    }
}

pub(crate) fn validate_native_block_slot_insert(
    model: &ProjectModel,
    context: &NativeBlockSlotMutationContext,
    target_source_id: Option<&str>,
) -> Result<(), String> {
    let slot = require_slot(model, context)?;
    if slot.container_source_node_id.as_deref() != target_source_id {
        return Err("Ținta inserării nu este containerul slotului Rust selectat.".to_string());
    }
    if slot
        .maximum_items
        .is_some_and(|maximum| slot.items.len() >= maximum)
    {
        return Err(format!(
            "Slotul `{}` acceptă cel mult {} elemente.",
            slot.id,
            slot.maximum_items.unwrap_or_default()
        ));
    }
    Ok(())
}

pub(crate) fn validate_native_block_slot_duplicate(
    model: &ProjectModel,
    context: &NativeBlockSlotMutationContext,
    source_id: Option<&str>,
) -> Result<(), String> {
    let slot = require_slot(model, context)?;
    require_slot_item(&slot, source_id)?;
    if slot
        .maximum_items
        .is_some_and(|maximum| slot.items.len() >= maximum)
    {
        return Err(format!(
            "Slotul `{}` acceptă cel mult {} elemente.",
            slot.id,
            slot.maximum_items.unwrap_or_default()
        ));
    }
    Ok(())
}

pub(crate) fn validate_native_block_slot_delete(
    model: &ProjectModel,
    context: &NativeBlockSlotMutationContext,
    source_id: Option<&str>,
) -> Result<(), String> {
    let slot = require_slot(model, context)?;
    require_slot_item(&slot, source_id)?;
    if slot.items.len() <= slot.minimum_items {
        return Err(format!(
            "Slotul `{}` trebuie să păstreze cel puțin {} element.",
            slot.id, slot.minimum_items
        ));
    }
    Ok(())
}

pub(crate) fn validate_native_block_slot_move(
    model: &ProjectModel,
    context: &NativeBlockSlotMutationContext,
    source_id: Option<&str>,
    target_id: Option<&str>,
    position: ProjectMovePosition,
) -> Result<(), String> {
    let slot = require_slot(model, context)?;
    require_slot_item(&slot, source_id)?;
    require_slot_item(&slot, target_id)?;
    if position == ProjectMovePosition::Inside {
        return Err(
            "Elementele unui slot pot fi reordonate doar înainte sau după alt element.".to_string(),
        );
    }
    Ok(())
}

pub(crate) fn node_has_native_block_ancestor(
    model: &ProjectModel,
    node: &SourceNode,
    provider_id: &str,
) -> bool {
    let mut parent_id = node.parent.as_deref();
    let mut visited = HashSet::new();
    while let Some(id) = parent_id {
        if !visited.insert(id) {
            break;
        }
        let Some(parent) = model.source_graph.node_by_id(id) else {
            break;
        };
        if node_is_native_block(model, parent, provider_id) {
            return true;
        }
        parent_id = parent.parent.as_deref();
    }
    false
}

pub(crate) fn node_subtree_contains_native_block(
    model: &ProjectModel,
    node: &SourceNode,
    provider_id: &str,
) -> bool {
    node_is_native_block(model, node, provider_id)
        || descendant_nodes(model, node)
            .into_iter()
            .any(|descendant| node_is_native_block(model, descendant, provider_id))
}

pub(crate) fn node_is_native_block(
    model: &ProjectModel,
    node: &SourceNode,
    provider_id: &str,
) -> bool {
    node.kind == SourceNodeKind::Html
        && node_attributes(model, node)
            .ok()
            .and_then(|attributes| {
                attribute_value(&attributes, "data-pana-block").map(str::to_string)
            })
            .as_deref()
            == Some(provider_id)
}

pub(crate) fn node_is_slider_slot_container(model: &ProjectModel, node: &SourceNode) -> bool {
    node.kind == SourceNodeKind::Html
        && node_attributes(model, node).is_ok_and(|attributes| {
            attributes.contains_key("data-pana-slider-track")
                && attribute_value(&attributes, "data-pana-slot") == Some("slides")
        })
}

pub(crate) fn node_is_slider_slot_item(model: &ProjectModel, node: &SourceNode) -> bool {
    node.kind == SourceNodeKind::Html
        && node_attributes(model, node)
            .is_ok_and(|attributes| attributes.contains_key("data-pana-slider-slide"))
}

pub(crate) fn node_is_slider_managed_scaffold(model: &ProjectModel, node: &SourceNode) -> bool {
    if node.kind != SourceNodeKind::Html || node_is_native_block(model, node, "slider") {
        return false;
    }
    node_attributes(model, node).is_ok_and(|attributes| {
        [
            "data-pana-slider-viewport",
            "data-pana-slider-track",
            "data-pana-slider-slide",
            "data-pana-slider-controls",
            "data-pana-slider-previous",
            "data-pana-slider-next",
            "data-pana-slider-indicators",
            "data-pana-slider-autoplay",
        ]
        .into_iter()
        .any(|attribute| attributes.contains_key(attribute))
    })
}

fn require_slot(
    model: &ProjectModel,
    context: &NativeBlockSlotMutationContext,
) -> Result<NativeBlockSlotState, String> {
    if model.revision != context.expected_model_revision.trim() {
        return Err(
            "Contractul slotului este stale față de Project Model-ul Rust curent.".to_string(),
        );
    }
    let root = model
        .source_graph
        .node_by_id(&context.root_source_id)
        .filter(|node| node.kind == SourceNodeKind::Html)
        .ok_or_else(|| "Rădăcina blocului nu mai există în Source Graph.".to_string())?;
    let attributes = node_attributes(model, root)?;
    if attribute_value(&attributes, "data-pana-block") != Some(context.provider_id.trim()) {
        return Err("Rădăcina nu mai aparține providerului declarat de intenție.".to_string());
    }
    inspect_native_block_slots(model, root, context.provider_id.trim())
        .into_iter()
        .find(|slot| slot.id == context.slot_id.trim())
        .ok_or_else(|| "Slotul cerut nu aparține providerului Rust.".to_string())
        .and_then(|slot| {
            if slot.editable {
                Ok(slot)
            } else {
                Err(slot
                    .diagnostic
                    .unwrap_or_else(|| "Contractul slotului este read-only.".to_string()))
            }
        })
}

fn require_slot_item(slot: &NativeBlockSlotState, source_id: Option<&str>) -> Result<(), String> {
    let source_id = source_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Intenția slotului nu identifică elementul sursă.".to_string())?;
    if slot
        .items
        .iter()
        .any(|item| item.source_node_id == source_id)
    {
        Ok(())
    } else {
        Err("Elementul nu aparține slotului Rust selectat.".to_string())
    }
}

fn inspect_slider_slides(model: &ProjectModel, root: &SourceNode) -> NativeBlockSlotState {
    let mut diagnostics = Vec::new();
    let descendants = descendant_nodes(model, root);
    let nested_slider = descendants.iter().any(|node| {
        node.kind == SourceNodeKind::Html
            && node_attributes(model, node)
                .ok()
                .and_then(|attrs| attribute_value(&attrs, "data-pana-block").map(str::to_string))
                .as_deref()
                == Some("slider")
    });
    if nested_slider {
        diagnostics.push("Slider în slider este blocat în contractul v1.".to_string());
    }

    let tracks = descendants
        .iter()
        .copied()
        .filter(|node| {
            node.kind == SourceNodeKind::Html
                && node_attributes(model, node).is_ok_and(|attrs| {
                    attribute_value(&attrs, "data-pana-slot") == Some("slides")
                        && attrs.contains_key("data-pana-slider-track")
                })
        })
        .collect::<Vec<_>>();
    if tracks.len() != 1 {
        diagnostics.push(
            "Sliderul trebuie să conțină exact un container pentru slotul `slides`.".to_string(),
        );
    }
    let track = tracks.first().copied();
    let mut items = Vec::new();
    if let Some(track) = track {
        for child_id in &track.children {
            let Some(child) = model
                .source_graph
                .node_by_id(child_id)
                .filter(|node| node.kind == SourceNodeKind::Html)
            else {
                continue;
            };
            match node_attributes(model, child) {
                Ok(attributes) if attributes.contains_key("data-pana-slider-slide") => {
                    let index = items.len();
                    items.push(NativeBlockSlotItemState {
                        source_node_id: child.id.clone(),
                        tag: "div".to_string(),
                        label: format!("Slide {}", index + 1),
                        index,
                        editable: child.capabilities.can_edit_visual,
                    });
                }
                _ => diagnostics.push(
                    "Containerul `slides` conține un copil care nu respectă contractul de slide."
                        .to_string(),
                ),
            }
        }
    }
    if items.len() < SLIDER_MIN_SLIDES || items.len() > SLIDER_MAX_SLIDES {
        diagnostics.push(format!(
            "Sliderul trebuie să conțină între {SLIDER_MIN_SLIDES} și {SLIDER_MAX_SLIDES} slide-uri."
        ));
    }
    if items.iter().any(|item| !item.editable) {
        diagnostics.push("Cel puțin un slide este read-only în Source Graph.".to_string());
    }
    NativeBlockSlotState {
        id: "slides".to_string(),
        item_kind: "slide".to_string(),
        container_source_node_id: track.map(|node| node.id.clone()),
        minimum_items: SLIDER_MIN_SLIDES,
        maximum_items: Some(SLIDER_MAX_SLIDES),
        editable: diagnostics.is_empty()
            && root.capabilities.can_edit_visual
            && track.is_some_and(|node| node.capabilities.can_edit_visual),
        diagnostic: (!diagnostics.is_empty()).then(|| diagnostics.join(" ")),
        items,
    }
}

fn descendant_nodes<'a>(model: &'a ProjectModel, root: &SourceNode) -> Vec<&'a SourceNode> {
    let by_id = model
        .source_graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut queue = root
        .children
        .iter()
        .map(String::as_str)
        .collect::<VecDeque<_>>();
    let mut visited = HashSet::new();
    let mut descendants = Vec::new();
    while let Some(id) = queue.pop_front() {
        if !visited.insert(id) {
            continue;
        }
        let Some(node) = by_id.get(id).copied() else {
            continue;
        };
        queue.extend(node.children.iter().map(String::as_str));
        descendants.push(node);
    }
    descendants
}

fn node_attributes(
    model: &ProjectModel,
    node: &SourceNode,
) -> Result<BTreeMap<String, Option<String>>, String> {
    let file = model
        .files
        .iter()
        .find(|file| file.relative_path == node.file)
        .ok_or_else(|| format!("Fișierul {} lipsește din Project Model.", node.file))?;
    let start = node
        .range
        .as_ref()
        .map(|range| range.start)
        .ok_or_else(|| "Nodul HTML nu are range stabil.".to_string())?;
    let opening = parse_html_tag_at(&file.contents, start)
        .ok_or_else(|| "Tagul HTML nu mai poate fi citit din sursa canonică.".to_string())?;
    let source = file
        .contents
        .get(opening.start..opening.end)
        .ok_or_else(|| "Range-ul tagului HTML este invalid.".to_string())?;
    Ok(raw_tag_attributes(source)
        .into_iter()
        .map(|attribute| (attribute.name, attribute.value))
        .collect())
}

fn attribute_value<'a>(
    attributes: &'a BTreeMap<String, Option<String>>,
    name: &str,
) -> Option<&'a str> {
    attributes.get(name).and_then(|value| value.as_deref())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::test_support::ProjectModelTestFixture;

    use super::*;

    #[test]
    fn rust_renderer_owns_new_slider_slide_markup() {
        let html = render_native_block_slot_item_html("slider", "slides").unwrap();
        assert!(html.contains("data-pana-slider-slide"));
        assert!(html.contains("role=\"group\""));
        assert!(render_native_block_slot_item_html("slider", "unknown").is_err());
    }

    #[test]
    fn slider_slot_state_and_typed_guards_enforce_membership_limits_and_staleness() {
        let root = unique_test_dir();
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), slider_markup(2, "")).unwrap();
        let model = fixture.build_model().unwrap();
        let slider_root = slider_root(&model);
        let slot = inspect_native_block_slots(&model, slider_root, "slider")
            .into_iter()
            .next()
            .unwrap();
        assert!(slot.editable, "{:?}", slot.diagnostic);
        assert_eq!(slot.items.len(), 2);
        let context = NativeBlockSlotMutationContext {
            provider_id: "slider".to_string(),
            slot_id: "slides".to_string(),
            root_source_id: slider_root.id.clone(),
            expected_model_revision: model.revision.clone(),
        };
        validate_native_block_slot_insert(
            &model,
            &context,
            slot.container_source_node_id.as_deref(),
        )
        .unwrap();
        validate_native_block_slot_duplicate(&model, &context, Some(&slot.items[0].source_node_id))
            .unwrap();
        validate_native_block_slot_delete(&model, &context, Some(&slot.items[0].source_node_id))
            .unwrap();
        validate_native_block_slot_move(
            &model,
            &context,
            Some(&slot.items[0].source_node_id),
            Some(&slot.items[1].source_node_id),
            ProjectMovePosition::After,
        )
        .unwrap();
        let mut stale = context.clone();
        stale.expected_model_revision = "stale".to_string();
        assert!(validate_native_block_slot_insert(
            &model,
            &stale,
            slot.container_source_node_id.as_deref(),
        )
        .unwrap_err()
        .contains("stale"));
        assert!(
            validate_native_block_slot_delete(&model, &context, Some("outside"))
                .unwrap_err()
                .contains("nu aparține")
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn slider_slot_rejects_last_delete_max_insert_and_nested_slider() {
        let root = unique_test_dir();
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), slider_markup(1, "")).unwrap();
        let model = fixture.build_model().unwrap();
        let single_root = slider_root(&model);
        let slot = inspect_native_block_slots(&model, single_root, "slider").remove(0);
        let context = NativeBlockSlotMutationContext {
            provider_id: "slider".to_string(),
            slot_id: "slides".to_string(),
            root_source_id: single_root.id.clone(),
            expected_model_revision: model.revision.clone(),
        };
        assert!(validate_native_block_slot_delete(
            &model,
            &context,
            Some(&slot.items[0].source_node_id),
        )
        .is_err());

        fixture.source("templates/index.html", slider_markup(SLIDER_MAX_SLIDES, ""));
        let max_model = fixture.build_model().unwrap();
        let max_root = slider_root(&max_model);
        let max_slot = inspect_native_block_slots(&max_model, max_root, "slider").remove(0);
        let max_context = NativeBlockSlotMutationContext {
            provider_id: "slider".to_string(),
            slot_id: "slides".to_string(),
            root_source_id: max_root.id.clone(),
            expected_model_revision: max_model.revision.clone(),
        };
        assert!(validate_native_block_slot_insert(
            &max_model,
            &max_context,
            max_slot.container_source_node_id.as_deref(),
        )
        .is_err());

        let nested = slider_markup(1, &slider_markup(1, ""));
        fixture.source("templates/index.html", nested);
        let nested_model = fixture.build_model().unwrap();
        let outer = slider_root(&nested_model);
        let nested_slot = inspect_native_block_slots(&nested_model, outer, "slider").remove(0);
        fs::remove_dir_all(&root).unwrap();
        assert!(!nested_slot.editable);
        assert!(nested_slot.diagnostic.unwrap().contains("Slider în slider"));
    }

    fn slider_root(model: &ProjectModel) -> &SourceNode {
        let marker = model
            .source_graph
            .block_graph
            .source_instances
            .iter()
            .find(|instance| instance.provider_id == "slider")
            .unwrap();
        let marker_node = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.id == marker.source_node_id)
            .unwrap();
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| Some(node.id.as_str()) == marker_node.parent.as_deref())
            .unwrap()
    }

    fn slider_markup(slides: usize, nested: &str) -> String {
        let items = (0..slides)
            .map(|index| {
                let content = if index == 0 { nested } else { "" };
                format!(
                    "      <div class=\"slider__slide\" data-pana-slider-slide><p>{index}</p>{content}</div>"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "<div class=\"slider ps-slider-test\" data-pana-block=\"slider\" data-pana-instance=\"slider-test\">\n  <div data-pana-slider-viewport>\n    <div class=\"slider__track\" data-pana-slider-track data-pana-slot=\"slides\">\n{items}\n    </div>\n  </div>\n</div>\n"
        )
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-slider-slots-{}-{stamp}",
            std::process::id()
        ))
    }
}

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use crate::{js::PageJsConfig, kernel::file_buffer_store::hash_text};

pub const MOTION_GRAPH_SCHEMA_VERSION: u32 = 1;
pub const MOTION_TIMELINE_STEP_TIMING_COMMAND: &str = "motion.timeline.stepTiming";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionTimelineStepTimingInput {
    pub config: PageJsConfig,
    #[serde(default)]
    pub timeline_id: Option<String>,
    #[serde(default)]
    pub step_id: Option<String>,
    #[serde(default)]
    pub step_index: Option<usize>,
    pub patch: MotionTimelineStepTimingPatch,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MotionTimelineStepTimingPatch {
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionGraphTransaction {
    pub schema_version: u32,
    pub id: String,
    pub command: String,
    pub target: String,
    pub timeline_id: String,
    pub step_id: String,
    pub forward_patch: MotionTimelineStepTimingPatch,
    pub reverse_patch: MotionTimelineStepTimingPatch,
    pub before_config_hash: String,
    pub after_config_hash: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionTimelineStepTimingReceipt {
    pub schema_version: u32,
    pub command: String,
    pub changed: bool,
    pub timeline_id: String,
    pub step_id: String,
    pub step_index: usize,
    pub before_step: Value,
    pub after_step: Value,
    pub after_config: PageJsConfig,
    pub transaction: Option<MotionGraphTransaction>,
    pub diagnostics: Vec<String>,
}

pub fn apply_motion_timeline_step_timing(
    input: MotionTimelineStepTimingInput,
) -> Result<MotionTimelineStepTimingReceipt, String> {
    validate_patch(&input.patch)?;
    apply_step_timing_patch(
        input.config,
        input.timeline_id,
        input.step_id,
        input.step_index,
        input.patch,
    )
}

pub fn undo_motion_graph_transaction(
    config: PageJsConfig,
    transaction: &MotionGraphTransaction,
) -> Result<PageJsConfig, String> {
    validate_transaction_kind(transaction)?;
    let current_hash = hash_config(&config)?;
    if current_hash != transaction.after_config_hash {
        return Err(
            "MotionGraph a refuzat undo: config-ul curent nu corespunde hash-ului after al tranzacției."
                .to_string(),
        );
    }

    let receipt = apply_step_timing_patch(
        config,
        Some(transaction.timeline_id.clone()),
        Some(transaction.step_id.clone()),
        None,
        transaction.reverse_patch.clone(),
    )?;
    let reverted_hash = hash_config(&receipt.after_config)?;
    if reverted_hash != transaction.before_config_hash {
        return Err(
            "MotionGraph a refuzat undo: rezultatul nu corespunde hash-ului before al tranzacției."
                .to_string(),
        );
    }
    Ok(receipt.after_config)
}

pub fn redo_motion_graph_transaction(
    config: PageJsConfig,
    transaction: &MotionGraphTransaction,
) -> Result<PageJsConfig, String> {
    validate_transaction_kind(transaction)?;
    let current_hash = hash_config(&config)?;
    if current_hash != transaction.before_config_hash {
        return Err(
            "MotionGraph a refuzat redo: config-ul curent nu corespunde hash-ului before al tranzacției."
                .to_string(),
        );
    }

    let receipt = apply_step_timing_patch(
        config,
        Some(transaction.timeline_id.clone()),
        Some(transaction.step_id.clone()),
        None,
        transaction.forward_patch.clone(),
    )?;
    let redone_hash = hash_config(&receipt.after_config)?;
    if redone_hash != transaction.after_config_hash {
        return Err(
            "MotionGraph a refuzat redo: rezultatul nu corespunde hash-ului after al tranzacției."
                .to_string(),
        );
    }
    Ok(receipt.after_config)
}

fn apply_step_timing_patch(
    config: PageJsConfig,
    timeline_id: Option<String>,
    step_id: Option<String>,
    step_index: Option<usize>,
    patch: MotionTimelineStepTimingPatch,
) -> Result<MotionTimelineStepTimingReceipt, String> {
    validate_patch(&patch)?;
    let before_config_hash = hash_config(&config)?;
    let mut after_config = config;
    let mut motion = after_config
        .motion
        .take()
        .ok_or_else(|| "MotionGraph nu are motion config pentru Page JS.".to_string())?;
    let active_item_id = motion
        .get("activeItemId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let items = motion
        .get_mut("items")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            "MotionGraph a refuzat comanda: motion.items lipsește sau nu este array.".to_string()
        })?;
    let timeline_index =
        resolve_timeline_index(items, timeline_id.as_deref(), active_item_id.as_deref())?;
    let timeline = items
        .get_mut(timeline_index)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            "MotionGraph a refuzat comanda: timeline-ul nu este obiect JSON.".to_string()
        })?;
    let resolved_timeline_id = string_field(timeline, "id")
        .ok_or_else(|| "MotionGraph a refuzat comanda: timeline-ul nu are id.".to_string())?;
    let steps = timeline
        .get_mut("steps")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            "MotionGraph a refuzat comanda: timeline.steps lipsește sau nu este array.".to_string()
        })?;
    let resolved_step_index = resolve_step_index(steps, step_id.as_deref(), step_index)?;
    let step = steps
        .get_mut(resolved_step_index)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "MotionGraph a refuzat comanda: step-ul nu este obiect JSON.".to_string())?;
    let resolved_step_id = string_field(step, "id")
        .ok_or_else(|| "MotionGraph a refuzat comanda: step-ul nu are id.".to_string())?;
    let before_step = Value::Object(step.clone());
    let reverse_patch = reverse_patch_for_step(step, &patch);
    let changed = apply_patch_to_step(step, &patch);
    let after_step = Value::Object(step.clone());

    after_config.motion = Some(motion);
    let after_config_hash = hash_config(&after_config)?;
    let transaction = if changed {
        Some(MotionGraphTransaction {
            schema_version: MOTION_GRAPH_SCHEMA_VERSION,
            id: transaction_id(
                &resolved_timeline_id,
                &resolved_step_id,
                &before_config_hash,
                &after_config_hash,
            ),
            command: MOTION_TIMELINE_STEP_TIMING_COMMAND.to_string(),
            target: format!("motion/{resolved_timeline_id}/steps/{resolved_step_id}"),
            timeline_id: resolved_timeline_id.clone(),
            step_id: resolved_step_id.clone(),
            forward_patch: patch.clone(),
            reverse_patch,
            before_config_hash,
            after_config_hash,
        })
    } else {
        None
    };

    Ok(MotionTimelineStepTimingReceipt {
        schema_version: MOTION_GRAPH_SCHEMA_VERSION,
        command: MOTION_TIMELINE_STEP_TIMING_COMMAND.to_string(),
        changed,
        timeline_id: resolved_timeline_id,
        step_id: resolved_step_id,
        step_index: resolved_step_index,
        before_step,
        after_step,
        after_config,
        transaction,
        diagnostics: Vec::new(),
    })
}

fn validate_patch(patch: &MotionTimelineStepTimingPatch) -> Result<(), String> {
    if patch.position.is_none() && patch.duration.is_none() {
        return Err(
            "MotionGraph a refuzat stepTiming: patch-ul nu conține position sau duration."
                .to_string(),
        );
    }
    if let Some(position) = patch.position.as_ref() {
        if position.contains('\0') {
            return Err(
                "MotionGraph a refuzat stepTiming: position conține caracter nul.".to_string(),
            );
        }
        if position.len() > 128 {
            return Err(
                "MotionGraph a refuzat stepTiming: position depășește 128 caractere.".to_string(),
            );
        }
    }
    Ok(())
}

fn validate_transaction_kind(transaction: &MotionGraphTransaction) -> Result<(), String> {
    if transaction.command != MOTION_TIMELINE_STEP_TIMING_COMMAND {
        return Err(format!(
            "MotionGraph a refuzat tranzacția: command incompatibil {}.",
            transaction.command
        ));
    }
    Ok(())
}

fn resolve_timeline_index(
    items: &[Value],
    timeline_id: Option<&str>,
    active_item_id: Option<&str>,
) -> Result<usize, String> {
    if let Some(id) = timeline_id.filter(|id| !id.trim().is_empty()) {
        return items
            .iter()
            .position(|item| is_timeline_with_id(item, id))
            .ok_or_else(|| format!("MotionGraph nu a găsit timeline-ul {id}."));
    }

    if let Some(active_id) = active_item_id.filter(|id| !id.trim().is_empty()) {
        if let Some(index) = items
            .iter()
            .position(|item| is_timeline_with_id(item, active_id))
        {
            return Ok(index);
        }
    }

    items
        .iter()
        .position(is_timeline)
        .ok_or_else(|| "MotionGraph nu a găsit niciun timeline în motion.items.".to_string())
}

fn resolve_step_index(
    steps: &[Value],
    step_id: Option<&str>,
    step_index: Option<usize>,
) -> Result<usize, String> {
    if let Some(id) = step_id.filter(|id| !id.trim().is_empty()) {
        return steps
            .iter()
            .position(|step| step.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| format!("MotionGraph nu a găsit step-ul {id}."));
    }

    let index = step_index
        .ok_or_else(|| "MotionGraph cere stepId sau stepIndex pentru stepTiming.".to_string())?;
    if index >= steps.len() {
        return Err(format!(
            "MotionGraph a refuzat stepTiming: stepIndex {index} este în afara timeline-ului."
        ));
    }
    Ok(index)
}

fn is_timeline(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("timeline")
}

fn is_timeline_with_id(value: &Value, id: &str) -> bool {
    is_timeline(value) && value.get("id").and_then(Value::as_str) == Some(id)
}

fn string_field(object: &Map<String, Value>, field: &str) -> Option<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn reverse_patch_for_step(
    step: &Map<String, Value>,
    patch: &MotionTimelineStepTimingPatch,
) -> MotionTimelineStepTimingPatch {
    MotionTimelineStepTimingPatch {
        position: patch.position.as_ref().map(|_| {
            step.get("position")
                .and_then(Value::as_str)
                .unwrap_or("0")
                .to_string()
        }),
        duration: patch
            .duration
            .map(|_| step.get("duration").and_then(Value::as_u64).unwrap_or(0)),
    }
}

fn apply_patch_to_step(
    step: &mut Map<String, Value>,
    patch: &MotionTimelineStepTimingPatch,
) -> bool {
    let mut changed = false;
    if let Some(position) = patch.position.as_ref() {
        let next = Value::String(position.clone());
        if step.get("position") != Some(&next) {
            step.insert("position".to_string(), next);
            changed = true;
        }
    }
    if let Some(duration) = patch.duration {
        let next = Value::Number(Number::from(duration));
        if step.get("duration") != Some(&next) {
            step.insert("duration".to_string(), next);
            changed = true;
        }
    }
    changed
}

fn hash_config(config: &PageJsConfig) -> Result<String, String> {
    serde_json::to_string(config)
        .map(|serialized| hash_text(&serialized))
        .map_err(|error| format!("MotionGraph nu a putut serializa PageJsConfig: {error}"))
}

fn transaction_id(
    timeline_id: &str,
    step_id: &str,
    before_config_hash: &str,
    after_config_hash: &str,
) -> String {
    let seed = format!("{MOTION_TIMELINE_STEP_TIMING_COMMAND}:{timeline_id}:{step_id}:{before_config_hash}:{after_config_hash}");
    format!("motion-step-timing-{}", hash_text(&seed))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn page_js_config() -> PageJsConfig {
        PageJsConfig {
            version: Some(1),
            components: Vec::new(),
            motion: Some(json!({
                "schemaVersion": 1,
                "animeVersion": "4.4.1",
                "activeItemId": "timeline-a",
                "items": [
                    { "id": "animation-a", "type": "animation" },
                    {
                        "id": "timeline-a",
                        "type": "timeline",
                        "duration": 1000,
                        "steps": [
                            {
                                "id": "step-a",
                                "type": "animation",
                                "label": "Intro",
                                "position": "0",
                                "duration": 400,
                                "lane": "track-main",
                                "targetItemId": "animation-a",
                                "callback": { "enabled": false, "label": "Timeline callback", "code": "" }
                            }
                        ]
                    }
                ]
            })),
        }
    }

    #[test]
    fn step_timing_applies_without_ui_and_returns_transaction() {
        let receipt = apply_motion_timeline_step_timing(MotionTimelineStepTimingInput {
            config: page_js_config(),
            timeline_id: Some("timeline-a".to_string()),
            step_id: Some("step-a".to_string()),
            step_index: None,
            patch: MotionTimelineStepTimingPatch {
                position: Some("250".to_string()),
                duration: Some(650),
            },
        })
        .expect("step timing receipt");

        assert!(receipt.changed);
        assert_eq!(receipt.command, MOTION_TIMELINE_STEP_TIMING_COMMAND);
        assert_eq!(receipt.timeline_id, "timeline-a");
        assert_eq!(receipt.step_id, "step-a");
        assert_eq!(receipt.after_step["position"], "250");
        assert_eq!(receipt.after_step["duration"], 650);
        let transaction = receipt.transaction.as_ref().expect("transaction");
        assert_eq!(transaction.command, MOTION_TIMELINE_STEP_TIMING_COMMAND);
        assert_eq!(transaction.forward_patch.position.as_deref(), Some("250"));
        assert_eq!(transaction.reverse_patch.position.as_deref(), Some("0"));
        assert_eq!(transaction.reverse_patch.duration, Some(400));
    }

    #[test]
    fn step_timing_can_be_undone_and_redone_by_transaction_hash_chain() {
        let receipt = apply_motion_timeline_step_timing(MotionTimelineStepTimingInput {
            config: page_js_config(),
            timeline_id: Some("timeline-a".to_string()),
            step_id: Some("step-a".to_string()),
            step_index: None,
            patch: MotionTimelineStepTimingPatch {
                position: Some("500".to_string()),
                duration: Some(800),
            },
        })
        .expect("step timing receipt");
        let transaction = receipt.transaction.as_ref().expect("transaction");

        let undone = undo_motion_graph_transaction(receipt.after_config.clone(), transaction)
            .expect("undo motion transaction");
        assert_eq!(
            hash_config(&undone).unwrap(),
            transaction.before_config_hash
        );

        let redone =
            redo_motion_graph_transaction(undone, transaction).expect("redo motion transaction");
        assert_eq!(hash_config(&redone).unwrap(), transaction.after_config_hash);
    }

    #[test]
    fn step_timing_blocks_empty_patch_before_mutating() {
        let error = apply_motion_timeline_step_timing(MotionTimelineStepTimingInput {
            config: page_js_config(),
            timeline_id: Some("timeline-a".to_string()),
            step_id: Some("step-a".to_string()),
            step_index: None,
            patch: MotionTimelineStepTimingPatch::default(),
        })
        .expect_err("empty patch should be rejected");

        assert!(error.contains("patch-ul nu conține position sau duration"));
    }

    #[test]
    fn step_timing_blocks_stale_undo_hash() {
        let first = apply_motion_timeline_step_timing(MotionTimelineStepTimingInput {
            config: page_js_config(),
            timeline_id: Some("timeline-a".to_string()),
            step_id: Some("step-a".to_string()),
            step_index: None,
            patch: MotionTimelineStepTimingPatch {
                position: Some("100".to_string()),
                duration: None,
            },
        })
        .expect("first transaction");
        let transaction = first.transaction.as_ref().expect("transaction");

        let second = apply_motion_timeline_step_timing(MotionTimelineStepTimingInput {
            config: first.after_config,
            timeline_id: Some("timeline-a".to_string()),
            step_id: Some("step-a".to_string()),
            step_index: None,
            patch: MotionTimelineStepTimingPatch {
                position: Some("200".to_string()),
                duration: None,
            },
        })
        .expect("second transaction");

        let error = undo_motion_graph_transaction(second.after_config, transaction)
            .expect_err("stale undo should be rejected");
        assert!(error.contains("hash-ului after"));
    }
}

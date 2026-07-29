use serde::{Deserialize, Serialize};

use crate::{
    js::{
        MotionAction, MotionBehavior, MotionCustomCode, MotionDocument, MotionInteraction,
        PageJsConfig,
    },
    kernel::file_buffer_store::hash_text,
};

pub const MOTION_MUTATION_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionMutationInput {
    pub config: PageJsConfig,
    pub mutation: MotionMutation,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MotionMutation {
    CreateInteraction {
        interaction: MotionInteraction,
    },
    UpdateInteraction {
        interaction: MotionInteraction,
    },
    DeleteInteraction {
        interaction_id: String,
    },
    InsertAction {
        interaction_id: String,
        index: usize,
        action: MotionAction,
    },
    UpdateAction {
        interaction_id: String,
        action: MotionAction,
    },
    DeleteAction {
        interaction_id: String,
        action_id: String,
    },
    SetActionTiming {
        interaction_id: String,
        action_id: String,
        #[serde(default)]
        start: Option<f64>,
        #[serde(default)]
        duration: Option<f64>,
    },
    ReorderAction {
        interaction_id: String,
        action_id: String,
        index: usize,
    },
    UpsertBehavior {
        behavior: MotionBehavior,
    },
    DeleteBehavior {
        behavior_id: String,
    },
    UpsertCustomCode {
        custom_code: MotionCustomCode,
    },
    DeleteCustomCode {
        custom_code_id: String,
    },
    ReplaceDocument {
        document: MotionDocument,
    },
}

impl MotionMutation {
    pub fn command_name(&self) -> &'static str {
        match self {
            Self::CreateInteraction { .. } => "motion.createInteraction",
            Self::UpdateInteraction { .. } => "motion.updateInteraction",
            Self::DeleteInteraction { .. } => "motion.deleteInteraction",
            Self::InsertAction { .. } => "motion.insertAction",
            Self::UpdateAction { .. } => "motion.updateAction",
            Self::DeleteAction { .. } => "motion.deleteAction",
            Self::SetActionTiming { .. } => "motion.setActionTiming",
            Self::ReorderAction { .. } => "motion.reorderAction",
            Self::UpsertBehavior { .. } => "motion.upsertBehavior",
            Self::DeleteBehavior { .. } => "motion.deleteBehavior",
            Self::UpsertCustomCode { .. } => "motion.upsertCustomCode",
            Self::DeleteCustomCode { .. } => "motion.deleteCustomCode",
            Self::ReplaceDocument { .. } => "motion.replaceDocument",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionMutationTransaction {
    pub schema_version: u32,
    pub id: String,
    pub command: String,
    pub before_config_hash: String,
    pub after_config_hash: String,
    pub forward: MotionMutation,
    pub inverse: MotionMutation,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionMutationReceipt {
    pub schema_version: u32,
    pub command: String,
    pub changed: bool,
    pub config: PageJsConfig,
    pub diagnostics: Vec<crate::js::MotionDiagnostic>,
    pub transaction: Option<MotionMutationTransaction>,
}

pub fn apply_motion_mutation(input: MotionMutationInput) -> Result<MotionMutationReceipt, String> {
    let before_hash = hash_config(&input.config)?;
    let mut config = input.config;
    config.version = Some(2);
    let mut document = config.motion.take().unwrap_or_default();
    let inverse = apply_mutation(&mut document, &input.mutation)?;
    document.validate()?;
    let diagnostics = document.diagnostics();
    config.motion = if document.is_empty() {
        None
    } else {
        Some(document)
    };
    let after_hash = hash_config(&config)?;
    let changed = before_hash != after_hash;
    let command = input.mutation.command_name().to_string();
    let transaction = changed.then(|| MotionMutationTransaction {
        schema_version: MOTION_MUTATION_SCHEMA_VERSION,
        id: transaction_id(&command, &before_hash, &after_hash),
        command: command.clone(),
        before_config_hash: before_hash,
        after_config_hash: after_hash,
        forward: input.mutation,
        inverse,
    });

    Ok(MotionMutationReceipt {
        schema_version: MOTION_MUTATION_SCHEMA_VERSION,
        command,
        changed,
        config,
        diagnostics,
        transaction,
    })
}

pub fn undo_motion_mutation(
    config: PageJsConfig,
    transaction: &MotionMutationTransaction,
) -> Result<MotionMutationReceipt, String> {
    let current_hash = hash_config(&config)?;
    if current_hash != transaction.after_config_hash {
        return Err(
            "Motion v2 a refuzat undo: configurația nu corespunde hash-ului after.".to_string(),
        );
    }
    let receipt = apply_motion_mutation(MotionMutationInput {
        config,
        mutation: transaction.inverse.clone(),
    })?;
    if hash_config(&receipt.config)? != transaction.before_config_hash {
        return Err(
            "Motion v2 a refuzat undo: rezultatul nu corespunde hash-ului before.".to_string(),
        );
    }
    Ok(receipt)
}

pub fn redo_motion_mutation(
    config: PageJsConfig,
    transaction: &MotionMutationTransaction,
) -> Result<MotionMutationReceipt, String> {
    let current_hash = hash_config(&config)?;
    if current_hash != transaction.before_config_hash {
        return Err(
            "Motion v2 a refuzat redo: configurația nu corespunde hash-ului before.".to_string(),
        );
    }
    let receipt = apply_motion_mutation(MotionMutationInput {
        config,
        mutation: transaction.forward.clone(),
    })?;
    if hash_config(&receipt.config)? != transaction.after_config_hash {
        return Err(
            "Motion v2 a refuzat redo: rezultatul nu corespunde hash-ului after.".to_string(),
        );
    }
    Ok(receipt)
}

fn apply_mutation(
    document: &mut MotionDocument,
    mutation: &MotionMutation,
) -> Result<MotionMutation, String> {
    match mutation {
        MotionMutation::CreateInteraction { interaction } => {
            if document
                .interactions
                .iter()
                .any(|candidate| candidate.id == interaction.id)
            {
                return Err(format!(
                    "Motion v2 nu poate crea interacțiunea {}: ID duplicat.",
                    interaction.id
                ));
            }
            document.interactions.push(interaction.clone());
            Ok(MotionMutation::DeleteInteraction {
                interaction_id: interaction.id.clone(),
            })
        }
        MotionMutation::UpdateInteraction { interaction } => {
            let current = interaction_mut(document, &interaction.id)?;
            let previous = current.clone();
            *current = interaction.clone();
            Ok(MotionMutation::UpdateInteraction {
                interaction: previous,
            })
        }
        MotionMutation::DeleteInteraction { interaction_id } => {
            let index = interaction_index(document, interaction_id)?;
            let interaction = document.interactions.remove(index);
            let referring_interaction_id = document
                .interactions
                .iter()
                .find(|candidate| {
                    candidate.actions.iter().any(|action| {
                        matches!(
                            action,
                            MotionAction::Nested(nested)
                                if nested.interaction_id == *interaction_id
                        )
                    })
                })
                .map(|candidate| candidate.id.clone());
            if let Some(referring_interaction_id) = referring_interaction_id {
                document.interactions.insert(index, interaction);
                return Err(format!(
                    "Motion v2 nu poate șterge interacțiunea {interaction_id}: este inclusă în {referring_interaction_id}."
                ));
            }
            Ok(MotionMutation::CreateInteraction { interaction })
        }
        MotionMutation::InsertAction {
            interaction_id,
            index,
            action,
        } => {
            let interaction = interaction_mut(document, interaction_id)?;
            if interaction
                .actions
                .iter()
                .any(|candidate| candidate.id() == action.id())
            {
                return Err(format!(
                    "Motion v2 nu poate adăuga acțiunea {}: ID duplicat.",
                    action.id()
                ));
            }
            let index = (*index).min(interaction.actions.len());
            interaction.actions.insert(index, action.clone());
            Ok(MotionMutation::DeleteAction {
                interaction_id: interaction_id.clone(),
                action_id: action.id().to_string(),
            })
        }
        MotionMutation::UpdateAction {
            interaction_id,
            action,
        } => {
            let interaction = interaction_mut(document, interaction_id)?;
            let index = action_index(interaction, action.id())?;
            let previous = std::mem::replace(&mut interaction.actions[index], action.clone());
            Ok(MotionMutation::UpdateAction {
                interaction_id: interaction_id.clone(),
                action: previous,
            })
        }
        MotionMutation::DeleteAction {
            interaction_id,
            action_id,
        } => {
            let interaction = interaction_mut(document, interaction_id)?;
            if interaction.actions.len() <= 1 {
                return Err(
                    "Motion v2 nu permite o interacțiune fără acțiuni; șterge interacțiunea."
                        .to_string(),
                );
            }
            let index = action_index(interaction, action_id)?;
            let action = interaction.actions.remove(index);
            Ok(MotionMutation::InsertAction {
                interaction_id: interaction_id.clone(),
                index,
                action,
            })
        }
        MotionMutation::SetActionTiming {
            interaction_id,
            action_id,
            start,
            duration,
        } => {
            if start.is_none() && duration.is_none() {
                return Err("Motion v2 setActionTiming cere start și/sau duration.".to_string());
            }
            let interaction = interaction_mut(document, interaction_id)?;
            let index = action_index(interaction, action_id)?;
            let action = &mut interaction.actions[index];
            let previous_start = start.map(|_| action.start());
            let previous_duration = duration.map(|_| action.duration());
            set_action_timing(action, *start, *duration)?;
            Ok(MotionMutation::SetActionTiming {
                interaction_id: interaction_id.clone(),
                action_id: action_id.clone(),
                start: previous_start,
                duration: previous_duration,
            })
        }
        MotionMutation::ReorderAction {
            interaction_id,
            action_id,
            index,
        } => {
            let interaction = interaction_mut(document, interaction_id)?;
            let previous_index = action_index(interaction, action_id)?;
            let action = interaction.actions.remove(previous_index);
            let next_index = (*index).min(interaction.actions.len());
            interaction.actions.insert(next_index, action);
            Ok(MotionMutation::ReorderAction {
                interaction_id: interaction_id.clone(),
                action_id: action_id.clone(),
                index: previous_index,
            })
        }
        MotionMutation::UpsertBehavior { behavior } => {
            if let Some(index) = document
                .behaviors
                .iter()
                .position(|candidate| candidate.id() == behavior.id())
            {
                let previous = std::mem::replace(&mut document.behaviors[index], behavior.clone());
                Ok(MotionMutation::UpsertBehavior { behavior: previous })
            } else {
                document.behaviors.push(behavior.clone());
                Ok(MotionMutation::DeleteBehavior {
                    behavior_id: behavior.id().to_string(),
                })
            }
        }
        MotionMutation::DeleteBehavior { behavior_id } => {
            let index = document
                .behaviors
                .iter()
                .position(|candidate| candidate.id() == behavior_id)
                .ok_or_else(|| format!("Motion v2 nu a găsit behavior-ul {behavior_id}."))?;
            let behavior = document.behaviors.remove(index);
            Ok(MotionMutation::UpsertBehavior { behavior })
        }
        MotionMutation::UpsertCustomCode { custom_code } => {
            if let Some(index) = document
                .custom_code
                .iter()
                .position(|candidate| candidate.id == custom_code.id)
            {
                let previous =
                    std::mem::replace(&mut document.custom_code[index], custom_code.clone());
                Ok(MotionMutation::UpsertCustomCode {
                    custom_code: previous,
                })
            } else {
                document.custom_code.push(custom_code.clone());
                Ok(MotionMutation::DeleteCustomCode {
                    custom_code_id: custom_code.id.clone(),
                })
            }
        }
        MotionMutation::DeleteCustomCode { custom_code_id } => {
            let index = document
                .custom_code
                .iter()
                .position(|candidate| candidate.id == *custom_code_id)
                .ok_or_else(|| {
                    format!("Motion v2 nu a găsit codul personalizat {custom_code_id}.")
                })?;
            let custom_code = document.custom_code.remove(index);
            Ok(MotionMutation::UpsertCustomCode { custom_code })
        }
        MotionMutation::ReplaceDocument {
            document: replacement,
        } => {
            let previous = std::mem::replace(document, replacement.clone());
            Ok(MotionMutation::ReplaceDocument { document: previous })
        }
    }
}

fn interaction_index(document: &MotionDocument, id: &str) -> Result<usize, String> {
    document
        .interactions
        .iter()
        .position(|interaction| interaction.id == id)
        .ok_or_else(|| format!("Motion v2 nu a găsit interacțiunea {id}."))
}

fn interaction_mut<'a>(
    document: &'a mut MotionDocument,
    id: &str,
) -> Result<&'a mut MotionInteraction, String> {
    let index = interaction_index(document, id)?;
    Ok(&mut document.interactions[index])
}

fn action_index(interaction: &MotionInteraction, id: &str) -> Result<usize, String> {
    interaction
        .actions
        .iter()
        .position(|action| action.id() == id)
        .ok_or_else(|| {
            format!(
                "Motion v2 nu a găsit acțiunea {id} în interacțiunea {}.",
                interaction.id
            )
        })
}

fn set_action_timing(
    action: &mut MotionAction,
    start: Option<f64>,
    duration: Option<f64>,
) -> Result<(), String> {
    if start.is_some_and(|value| !value.is_finite() || value < 0.0)
        || duration.is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err("Motion v2 cere timing finit și nenegativ.".to_string());
    }
    match action {
        MotionAction::Animate(action) => {
            if let Some(start) = start {
                action.start = start;
            }
            if let Some(duration) = duration {
                if duration <= 0.0 {
                    return Err("Acțiunea Animate trebuie să aibă durată pozitivă.".to_string());
                }
                action.duration = duration;
            }
        }
        MotionAction::Nested(action) => {
            if let Some(start) = start {
                action.start = start;
            }
            if let Some(duration) = duration {
                action.duration = duration;
            }
        }
        MotionAction::Set(action) => {
            if duration.is_some() {
                return Err("Acțiunea Set are întotdeauna durata 0.".to_string());
            }
            if let Some(start) = start {
                action.start = start;
            }
        }
        MotionAction::Media(action) => {
            if duration.is_some() {
                return Err("Acțiunea Media este instantanee.".to_string());
            }
            if let Some(start) = start {
                action.start = start;
            }
        }
        MotionAction::Call(action) => {
            if duration.is_some() {
                return Err("Acțiunea Call este instantanee.".to_string());
            }
            if let Some(start) = start {
                action.start = start;
            }
        }
    }
    Ok(())
}

fn hash_config(config: &PageJsConfig) -> Result<String, String> {
    serde_json::to_string(config)
        .map(|source| hash_text(&source))
        .map_err(|error| format!("Motion v2 nu a putut serializa configurația: {error}"))
}

fn transaction_id(command: &str, before_hash: &str, after_hash: &str) -> String {
    format!(
        "motion-v2-{}",
        hash_text(&format!("{command}:{before_hash}:{after_hash}"))
    )
}

#[cfg(test)]
mod tests {
    use crate::js::motion_model::{
        MotionActionRepeat, MotionAnimateAction, MotionAnimationMode, MotionConditions,
        MotionInteraction, MotionPlayback, MotionProperty, MotionPropertyCategory, MotionTarget,
        MotionTimelineDomain, MotionTrigger, MotionValue, MotionValueKind,
    };

    use super::*;

    fn interaction() -> MotionInteraction {
        MotionInteraction {
            id: "hero".to_string(),
            name: "Hero".to_string(),
            enabled: true,
            trigger: MotionTrigger::default(),
            trigger_target: MotionTarget::for_data_anim("hero"),
            conditions: MotionConditions::default(),
            playback: MotionPlayback::default(),
            domain: MotionTimelineDomain::Time,
            actions: vec![MotionAction::Animate(MotionAnimateAction {
                id: "fade".to_string(),
                name: "Fade".to_string(),
                enabled: true,
                target: MotionTarget::for_data_anim("hero"),
                start: 0.0,
                duration: 600.0,
                mode: MotionAnimationMode::To,
                ease: "out(3)".to_string(),
                properties: vec![MotionProperty {
                    id: "opacity".to_string(),
                    name: "opacity".to_string(),
                    category: MotionPropertyCategory::Style,
                    from: None,
                    to: MotionValue {
                        kind: MotionValueKind::Number,
                        value: "1".to_string(),
                        unit: String::new(),
                    },
                }],
                keyframes: Vec::new(),
                stagger: None,
                repeat: MotionActionRepeat::default(),
                specialization: None,
            })],
            markers: Vec::new(),
        }
    }

    fn empty_config() -> PageJsConfig {
        PageJsConfig {
            version: Some(2),
            blocks: Vec::new(),
            motion: None,
        }
    }

    #[test]
    fn mutation_wire_contract_uses_camel_case_for_variant_fields() {
        let mutation: MotionMutation = serde_json::from_value(serde_json::json!({
            "command": "setActionTiming",
            "interactionId": "hero",
            "actionId": "fade",
            "start": 125.0,
            "duration": 750.0
        }))
        .expect("camelCase mutation");

        assert!(matches!(
            mutation,
            MotionMutation::SetActionTiming {
                ref interaction_id,
                ref action_id,
                start: Some(125.0),
                duration: Some(750.0),
            } if interaction_id == "hero" && action_id == "fade"
        ));

        let serialized = serde_json::to_value(mutation).expect("serialize mutation");
        assert_eq!(serialized["interactionId"], "hero");
        assert_eq!(serialized["actionId"], "fade");
        assert!(serialized.get("interaction_id").is_none());
        assert!(serialized.get("action_id").is_none());
    }

    #[test]
    fn create_interaction_returns_an_inverse_transaction() {
        let receipt = apply_motion_mutation(MotionMutationInput {
            config: empty_config(),
            mutation: MotionMutation::CreateInteraction {
                interaction: interaction(),
            },
        })
        .expect("create");
        assert!(receipt.changed);
        let transaction = receipt.transaction.expect("transaction");
        assert!(matches!(
            transaction.inverse,
            MotionMutation::DeleteInteraction { .. }
        ));
    }

    #[test]
    fn timing_is_atomic_and_undoable() {
        let created = apply_motion_mutation(MotionMutationInput {
            config: empty_config(),
            mutation: MotionMutation::CreateInteraction {
                interaction: interaction(),
            },
        })
        .expect("create");
        let moved = apply_motion_mutation(MotionMutationInput {
            config: created.config,
            mutation: MotionMutation::SetActionTiming {
                interaction_id: "hero".to_string(),
                action_id: "fade".to_string(),
                start: Some(250.0),
                duration: Some(900.0),
            },
        })
        .expect("move");
        let transaction = moved.transaction.as_ref().expect("transaction");
        let undone = undo_motion_mutation(moved.config.clone(), transaction).expect("undo");
        let action = &undone.config.motion.as_ref().expect("motion").interactions[0].actions[0];
        assert_eq!(action.start(), 0.0);
        assert_eq!(action.duration(), 600.0);
        let redone = redo_motion_mutation(undone.config, transaction).expect("redo");
        assert_eq!(
            redone.config.motion.expect("motion").interactions[0].actions[0].start(),
            250.0
        );
    }

    #[test]
    fn refuses_to_delete_last_action() {
        let created = apply_motion_mutation(MotionMutationInput {
            config: empty_config(),
            mutation: MotionMutation::CreateInteraction {
                interaction: interaction(),
            },
        })
        .expect("create");
        let error = apply_motion_mutation(MotionMutationInput {
            config: created.config,
            mutation: MotionMutation::DeleteAction {
                interaction_id: "hero".to_string(),
                action_id: "fade".to_string(),
            },
        })
        .unwrap_err();
        assert!(error.contains("fără acțiuni"));
    }
}

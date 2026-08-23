pub(crate) mod contract;
pub(crate) mod graph;
pub(crate) mod icons;
pub(crate) mod native;
pub(crate) mod options;
pub(crate) mod runtime;
pub(crate) mod slots;

pub(crate) use contract::{plan_native_block_contract, NativeBlockContractRequest};
pub(crate) use icons::{
    inspect_native_icon_source, read_icon_catalog, search_icon_catalog, IconCatalogPage,
    IconCatalogSearchInput, IconCatalogSummary, NativeIconMutationIntent, NativeIconState,
};
pub(crate) use native::{
    native_block_by_id, native_block_contract_definition, native_block_instance_id,
    native_block_provider_definitions, native_block_registry_snapshot,
    native_block_root_class_name, render_native_block_html, unique_native_block_identity,
    NativeBlockRegistrySnapshot,
};
pub(crate) use options::{
    inspect_native_block_source, plan_native_block_option_attribute, NativeBlockOptionIntent,
    NativeBlockOptionState,
};
pub(crate) use runtime::{render_native_block_runtime, NativeBlockRuntimePlan};
pub(crate) use slots::{
    inspect_native_block_slots, node_has_native_block_ancestor, node_is_native_block,
    node_is_slider_managed_scaffold, node_is_slider_slot_container, node_is_slider_slot_item,
    node_subtree_contains_native_block, render_native_block_slot_item_html,
    validate_native_block_slot_delete, validate_native_block_slot_duplicate,
    validate_native_block_slot_insert, validate_native_block_slot_move,
    NativeBlockSlotMutationContext, NativeBlockSlotState,
};

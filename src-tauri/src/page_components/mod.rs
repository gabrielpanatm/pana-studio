mod contract;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use serde::Serialize;

pub use contract::{
    plan_page_component_contract, PageComponentContractPlan, PageComponentContractRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PageComponentKind {
    Js,
}

#[derive(Clone, Copy, Debug)]
pub struct PageComponentDefinition {
    pub id: &'static str,
    pub kind: PageComponentKind,
    pub label: &'static str,
    pub description: &'static str,
    pub tag: &'static str,
    pub class_name: &'static str,
    pub text: &'static str,
    pub html: &'static str,
    pub scss: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageComponentRegistryItem {
    pub id: &'static str,
    pub kind: PageComponentKind,
    pub label: &'static str,
    pub description: &'static str,
    pub tag: &'static str,
    pub text: &'static str,
    pub class_name: &'static str,
    pub html: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageComponentRegistryGroup {
    pub label: &'static str,
    pub elements: Vec<PageComponentRegistryItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageComponentRegistrySnapshot {
    pub schema_version: u32,
    pub components: Vec<PageComponentRegistryItem>,
    pub groups: Vec<PageComponentRegistryGroup>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageComponentIdentity {
    pub class_name: String,
    pub data_anim: String,
    pub instance_id: String,
}

const COMPONENTS: &[PageComponentDefinition] = &[
    PageComponentDefinition {
        id: "counter",
        kind: PageComponentKind::Js,
        label: "Counter",
        description: "numar animat la scroll",
        tag: "span",
        text: "0",
        class_name: "counter",
        html: r#"<span class="counter __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-component="counter" data-pana-instance="__PANA_INSTANCE__" data-tinta="1250" data-sufix="+">0</span>"#,
        scss: r#".counter {
  font-variant-numeric: tabular-nums;
}"#,
    },
    PageComponentDefinition {
        id: "accordion",
        kind: PageComponentKind::Js,
        label: "Accordion",
        description: "sectiuni expandabile",
        tag: "div",
        text: "",
        class_name: "accordion",
        html: r#"<div class="accordion __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-component="accordion" data-pana-instance="__PANA_INSTANCE__">
  <div class="accordion__item" data-pana-accordion-item data-open>
    <button class="accordion__trigger" data-pana-accordion-trigger type="button" aria-expanded="true">
      <span>Prima intrebare</span>
      <span class="accordion__icon" aria-hidden="true">+</span>
    </button>
    <div class="accordion__panel" data-pana-accordion-panel>
      <p>Raspunsul poate fi editat direct in preview sau in cod.</p>
    </div>
  </div>
  <div class="accordion__item" data-pana-accordion-item>
    <button class="accordion__trigger" data-pana-accordion-trigger type="button" aria-expanded="false">
      <span>A doua intrebare</span>
      <span class="accordion__icon" aria-hidden="true">+</span>
    </button>
    <div class="accordion__panel" data-pana-accordion-panel hidden>
      <p>Adauga continutul tau aici.</p>
    </div>
  </div>
</div>"#,
        scss: r#".accordion {
  border: 1px solid #e5e7eb;
  border-radius: 0.75rem;
  background: #ffffff;
  overflow: hidden;
}

.accordion__item + .accordion__item {
  border-top: 1px solid #e5e7eb;
}

.accordion__trigger {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 1.125rem;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  font-weight: 700;
  text-align: left;
  cursor: pointer;
}

.accordion__trigger:focus-visible {
  outline: 2px solid #3b82f6;
  outline-offset: -2px;
}

.accordion__icon {
  transition: transform 180ms ease;
}

.accordion__item[data-open] .accordion__icon {
  transform: rotate(45deg);
}

.accordion__panel {
  padding: 0 1.125rem 1rem;
  color: #4b5563;
}

.accordion__panel > :first-child {
  margin-top: 0;
}

.accordion__panel > :last-child {
  margin-bottom: 0;
}

.accordion__panel[hidden] {
  display: none;
}"#,
    },
    PageComponentDefinition {
        id: "tabs",
        kind: PageComponentKind::Js,
        label: "Tabs",
        description: "panouri comutabile",
        tag: "div",
        text: "",
        class_name: "tabs",
        html: r#"<div class="tabs __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-component="tabs" data-pana-instance="__PANA_INSTANCE__">
  <div class="tabs__list" data-pana-tabs-list role="tablist" aria-label="Sectiuni">
    <button class="tabs__tab" data-pana-tabs-tab type="button" role="tab" aria-selected="true">Prezentare</button>
    <button class="tabs__tab" data-pana-tabs-tab type="button" role="tab" aria-selected="false">Detalii</button>
    <button class="tabs__tab" data-pana-tabs-tab type="button" role="tab" aria-selected="false">Proces</button>
  </div>
  <div class="tabs__panel" data-pana-tabs-panel role="tabpanel">
    <p>Continutul primului tab poate fi editat direct in preview sau in cod.</p>
  </div>
  <div class="tabs__panel" data-pana-tabs-panel role="tabpanel" hidden>
    <p>Adauga aici detaliile relevante.</p>
  </div>
  <div class="tabs__panel" data-pana-tabs-panel role="tabpanel" hidden>
    <p>Descrie pasii sau procesul aici.</p>
  </div>
</div>"#,
        scss: r#".tabs {
  border: 1px solid #e5e7eb;
  border-radius: 0.75rem;
  background: #ffffff;
  overflow: hidden;
}

.tabs__list {
  display: flex;
  gap: 0.25rem;
  padding: 0.35rem;
  border-bottom: 1px solid #e5e7eb;
  background: #f9fafb;
  overflow-x: auto;
}

.tabs__tab {
  border: 0;
  border-radius: 0.5rem;
  background: transparent;
  color: #4b5563;
  font: inherit;
  font-weight: 700;
  padding: 0.7rem 1rem;
  white-space: nowrap;
  cursor: pointer;
}

.tabs__tab[aria-selected="true"] {
  background: #ffffff;
  color: #111827;
  box-shadow: 0 1px 3px rgba(15, 23, 42, 0.12);
}

.tabs__tab:focus-visible {
  outline: 2px solid #3b82f6;
  outline-offset: 2px;
}

.tabs__panel {
  padding: 1rem 1.125rem;
  color: #4b5563;
}

.tabs__panel > :first-child {
  margin-top: 0;
}

.tabs__panel > :last-child {
  margin-bottom: 0;
}

.tabs__panel[hidden] {
  display: none;
}"#,
    },
    PageComponentDefinition {
        id: "dialog",
        kind: PageComponentKind::Js,
        label: "Dialog",
        description: "dialog cu trigger",
        tag: "div",
        text: "",
        class_name: "dialog",
        html: r#"<div class="dialog __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-component="dialog" data-pana-instance="__PANA_INSTANCE__">
  <button class="dialog__trigger" data-pana-dialog-open type="button" aria-haspopup="dialog" aria-expanded="false">
    Deschide dialog
  </button>
  <div class="dialog__overlay" data-pana-dialog-overlay hidden>
    <div class="dialog__panel" data-pana-dialog-panel role="dialog" aria-modal="true" tabindex="-1">
      <button class="dialog__close" data-pana-dialog-close type="button" aria-label="Inchide dialog">Inchide</button>
      <h2 class="dialog__title" data-pana-dialog-title>Titlu dialog</h2>
      <p>Continutul dialogului poate fi editat direct in preview sau in cod.</p>
      <div class="dialog__actions">
        <button class="dialog__button" data-pana-dialog-close type="button">Am inteles</button>
      </div>
    </div>
  </div>
</div>"#,
        scss: r#".dialog {
  display: inline-block;
}

.dialog__trigger,
.dialog__button {
  border: 0;
  border-radius: 0.5rem;
  background: #111827;
  color: #ffffff;
  font: inherit;
  font-weight: 700;
  padding: 0.75rem 1rem;
  cursor: pointer;
}

.dialog__trigger:focus-visible,
.dialog__button:focus-visible,
.dialog__close:focus-visible {
  outline: 2px solid #3b82f6;
  outline-offset: 2px;
}

.dialog__overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: grid;
  place-items: center;
  padding: 1rem;
  background: rgba(17, 24, 39, 0.55);
}

.dialog__overlay[hidden] {
  display: none;
}

.dialog__panel {
  width: min(100%, 34rem);
  max-height: min(90vh, 48rem);
  overflow: auto;
  border-radius: 0.875rem;
  background: #ffffff;
  color: #111827;
  padding: 1.25rem;
  box-shadow: 0 24px 60px rgba(15, 23, 42, 0.28);
}

.dialog__close {
  float: right;
  border: 0;
  border-radius: 0.375rem;
  background: #f3f4f6;
  color: #374151;
  font: inherit;
  padding: 0.45rem 0.65rem;
  cursor: pointer;
}

.dialog__title {
  margin: 0 2.5rem 0.75rem 0;
}

.dialog__panel > :last-child {
  margin-bottom: 0;
}

.dialog__actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  margin-top: 1rem;
}"#,
    },
    PageComponentDefinition {
        id: "offcanvas",
        kind: PageComponentKind::Js,
        label: "Offcanvas",
        description: "panou lateral glisant",
        tag: "div",
        text: "",
        class_name: "offcanvas",
        html: r#"<div class="offcanvas __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-component="offcanvas" data-pana-instance="__PANA_INSTANCE__" data-pana-offcanvas-side="end">
  <button class="offcanvas__trigger" data-pana-offcanvas-open type="button" aria-haspopup="dialog" aria-expanded="false">
    Deschide panoul
  </button>
  <div class="offcanvas__overlay" data-pana-offcanvas-overlay hidden>
    <aside class="offcanvas__panel" data-pana-offcanvas-panel role="dialog" aria-modal="true" tabindex="-1">
      <button class="offcanvas__close" data-pana-offcanvas-close type="button" aria-label="Inchide panoul">Inchide</button>
      <h2 class="offcanvas__title" data-pana-offcanvas-title>Panou lateral</h2>
      <p>Continutul panoului poate fi editat direct in preview sau in cod.</p>
      <div class="offcanvas__actions">
        <button class="offcanvas__button" data-pana-offcanvas-close type="button">Am inteles</button>
      </div>
    </aside>
  </div>
</div>"#,
        scss: r#".offcanvas {
  display: inline-block;
}

.offcanvas__trigger,
.offcanvas__button {
  border: 0;
  border-radius: 0.5rem;
  background: #111827;
  color: #ffffff;
  font: inherit;
  font-weight: 700;
  padding: 0.75rem 1rem;
  cursor: pointer;
}

.offcanvas__trigger:focus-visible,
.offcanvas__button:focus-visible,
.offcanvas__close:focus-visible {
  outline: 2px solid #3b82f6;
  outline-offset: 2px;
}

.offcanvas__overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  justify-content: flex-end;
  background: rgba(17, 24, 39, 0.45);
}

.offcanvas__overlay[hidden] {
  display: none;
}

.offcanvas__panel {
  width: min(28rem, 92vw);
  height: 100%;
  overflow: auto;
  background: #ffffff;
  color: #111827;
  padding: 1.25rem;
  box-shadow: -18px 0 48px rgba(15, 23, 42, 0.24);
  transform: translateX(100%);
  transition: transform 220ms ease;
}

.offcanvas[data-open] .offcanvas__panel {
  transform: translateX(0);
}

.offcanvas[data-pana-offcanvas-side="start"] .offcanvas__overlay {
  justify-content: flex-start;
}

.offcanvas[data-pana-offcanvas-side="start"] .offcanvas__panel {
  box-shadow: 18px 0 48px rgba(15, 23, 42, 0.24);
  transform: translateX(-100%);
}

.offcanvas[data-pana-offcanvas-side="start"][data-open] .offcanvas__panel {
  transform: translateX(0);
}

.offcanvas__close {
  float: right;
  border: 0;
  border-radius: 0.375rem;
  background: #f3f4f6;
  color: #374151;
  font: inherit;
  padding: 0.45rem 0.65rem;
  cursor: pointer;
}

.offcanvas__title {
  margin: 0 2.5rem 0.75rem 0;
}

.offcanvas__panel > :last-child {
  margin-bottom: 0;
}

.offcanvas__actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  margin-top: 1rem;
}"#,
    },
    PageComponentDefinition {
        id: "nav-menu",
        kind: PageComponentKind::Js,
        label: "Meniu navigatie",
        description: "meniu responsive cu toggle",
        tag: "nav",
        text: "",
        class_name: "nav-menu",
        html: r#"<nav class="nav-menu __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-component="nav-menu" data-pana-instance="__PANA_INSTANCE__" aria-label="Navigatie principala">
  <div class="nav-menu__bar">
    <a class="nav-menu__brand" href="/">Pana Site</a>
    <button class="nav-menu__toggle" data-pana-nav-menu-toggle type="button" aria-expanded="false">
      <span class="nav-menu__toggle-label">Meniu</span>
      <span class="nav-menu__toggle-icon" aria-hidden="true">
        <span></span>
        <span></span>
        <span></span>
      </span>
    </button>
  </div>
  <ul class="nav-menu__list" data-pana-nav-menu-list>
    <li><a class="nav-menu__link" href="/">Acasa</a></li>
    <li><a class="nav-menu__link" href="/servicii/">Servicii</a></li>
    <li><a class="nav-menu__link" href="/despre/">Despre</a></li>
    <li><a class="nav-menu__link" href="/contact/">Contact</a></li>
  </ul>
</nav>"#,
        scss: r#".nav-menu {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  width: 100%;
  padding: 0.75rem 1rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.75rem;
  background: #ffffff;
  color: #111827;
}

.nav-menu__bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.nav-menu__brand {
  color: inherit;
  font-weight: 800;
  text-decoration: none;
}

.nav-menu__toggle {
  display: none;
  align-items: center;
  gap: 0.55rem;
  border: 1px solid #d1d5db;
  border-radius: 0.5rem;
  background: #ffffff;
  color: inherit;
  font: inherit;
  font-weight: 700;
  padding: 0.55rem 0.75rem;
  cursor: pointer;
}

.nav-menu__toggle:focus-visible,
.nav-menu__link:focus-visible {
  outline: 2px solid #3b82f6;
  outline-offset: 2px;
}

.nav-menu__toggle-icon {
  display: inline-grid;
  gap: 0.2rem;
}

.nav-menu__toggle-icon span {
  display: block;
  width: 1rem;
  height: 2px;
  border-radius: 999px;
  background: currentColor;
}

.nav-menu__list {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  list-style: none;
  margin: 0;
  padding: 0;
}

.nav-menu__list[hidden] {
  display: flex;
}

.nav-menu__link {
  display: inline-flex;
  align-items: center;
  min-height: 2.35rem;
  border-radius: 0.5rem;
  color: #374151;
  font-weight: 700;
  text-decoration: none;
  padding: 0.45rem 0.7rem;
}

.nav-menu__link:hover {
  background: #f3f4f6;
  color: #111827;
}

@media (max-width: 720px) {
  .nav-menu {
    display: block;
  }

  .nav-menu__toggle {
    display: inline-flex;
  }

  .nav-menu__list,
  .nav-menu__list[hidden] {
    display: none;
  }

  .nav-menu[data-open] .nav-menu__list {
    display: grid;
    gap: 0.25rem;
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid #e5e7eb;
  }

  .nav-menu__link {
    width: 100%;
  }
}"#,
    },
];

pub fn page_component_by_id(id: &str) -> Option<&'static PageComponentDefinition> {
    let normalized = id.trim();
    COMPONENTS
        .iter()
        .find(|component| component.id == normalized)
}

pub fn known_page_component_ids() -> impl Iterator<Item = &'static str> {
    COMPONENTS.iter().map(|component| component.id)
}

pub fn page_component_registry_snapshot() -> PageComponentRegistrySnapshot {
    let components = COMPONENTS
        .iter()
        .map(PageComponentRegistryItem::from_definition)
        .collect::<Vec<_>>();
    let js_components = components
        .iter()
        .filter(|component| component.kind == PageComponentKind::Js)
        .cloned()
        .collect::<Vec<_>>();

    PageComponentRegistrySnapshot {
        schema_version: 1,
        components,
        groups: vec![PageComponentRegistryGroup {
            label: "JS",
            elements: js_components,
        }],
    }
}

pub fn page_component_preview_css<'a>(ids: impl IntoIterator<Item = &'a str>) -> String {
    ids.into_iter()
        .filter_map(page_component_by_id)
        .map(|component| component.scss.trim())
        .filter(|scss| !scss.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn render_page_component_html(
    component: &PageComponentDefinition,
    identity: &PageComponentIdentity,
) -> String {
    component
        .html
        .replace("__PANA_CLASS__", &identity.class_name)
        .replace("__PANA_DATA_ANIM__", &identity.data_anim)
        .replace("__PANA_INSTANCE__", &identity.instance_id)
}

pub fn component_root_class_name(
    component: &PageComponentDefinition,
    identity: &PageComponentIdentity,
) -> String {
    [component.class_name, identity.class_name.as_str()]
        .into_iter()
        .filter(|token| !token.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn unique_page_component_identity<F>(
    component_id: &str,
    seed: &str,
    mut exists: F,
) -> PageComponentIdentity
where
    F: FnMut(&str) -> bool,
{
    let component_token = normalize_component_token(component_id);
    for attempt in 0..80u32 {
        let token = identity_token(seed, attempt);
        let candidate = format!("ps-{component_token}-{token}");
        if !exists(&candidate) {
            return PageComponentIdentity {
                class_name: candidate.clone(),
                data_anim: candidate.clone(),
                instance_id: page_component_instance_id(component_id, &candidate),
            };
        }
    }

    let fallback = format!("ps-{component_token}-{}", identity_token(seed, 80));
    PageComponentIdentity {
        class_name: fallback.clone(),
        data_anim: fallback.clone(),
        instance_id: page_component_instance_id(component_id, &fallback),
    }
}

pub(crate) fn page_component_instance_id(component_id: &str, unique_token: &str) -> String {
    let trimmed = unique_token
        .strip_prefix("ps-")
        .unwrap_or(unique_token)
        .trim();
    format!("{}-{}", component_id.trim(), trimmed)
}

fn normalize_component_token(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_dash = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if character == '-' && !last_was_dash && !output.is_empty() {
            output.push('-');
            last_was_dash = true;
        }
    }
    while output.ends_with('-') {
        output.pop();
    }
    if output.is_empty() {
        "component".to_string()
    } else {
        output
    }
}

fn identity_token(seed: &str, attempt: u32) -> String {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    attempt.hash(&mut hasher);
    format!("{:08x}", (hasher.finish() & 0xffff_ffff) as u32)
}

impl PageComponentRegistryItem {
    fn from_definition(component: &PageComponentDefinition) -> Self {
        Self {
            id: component.id,
            kind: component.kind,
            label: component.label,
            description: component.description,
            tag: component.tag,
            text: component.text,
            class_name: component.class_name,
            html: component.html,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_component_html_with_kernel_identity() {
        let component = page_component_by_id("counter").unwrap();
        let identity = PageComponentIdentity {
            class_name: "ps-counter-12345678".to_string(),
            data_anim: "ps-counter-12345678".to_string(),
            instance_id: "counter-counter-12345678".to_string(),
        };

        let html = render_page_component_html(component, &identity);

        assert!(html.contains(r#"data-pana-component="counter""#));
        assert!(html.contains(r#"class="counter ps-counter-12345678""#));
        assert!(html.contains(r#"data-pana-instance="counter-counter-12345678""#));
        assert!(!html.contains("__PANA_"));
    }

    #[test]
    fn generated_identity_skips_collisions() {
        let identity = unique_page_component_identity("nav-menu", "seed", |candidate| {
            candidate.ends_with("38d91c63")
        });

        assert!(identity.class_name.starts_with("ps-nav-menu-"));
        assert_eq!(identity.class_name, identity.data_anim);
        assert!(identity.instance_id.starts_with("nav-menu-nav-menu-"));
    }

    #[test]
    fn registry_snapshot_exports_html_palette_contract() {
        let snapshot = page_component_registry_snapshot();
        let serialized_kind = serde_json::to_string(&PageComponentKind::Js).unwrap();

        assert_eq!(serialized_kind, "\"js\"");
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.components.len(), COMPONENTS.len());
        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!(snapshot.groups[0].label, "JS");
        assert_eq!(snapshot.groups[0].elements.len(), COMPONENTS.len());

        for component in snapshot.components {
            assert!(!component.id.trim().is_empty());
            assert!(!component.label.trim().is_empty());
            assert!(!component.description.trim().is_empty());
            assert!(component.html.contains("data-pana-component="));
            assert!(component.html.contains("__PANA_CLASS__"));
            assert_eq!(
                page_component_by_id(component.id).unwrap().tag,
                component.tag
            );
        }
    }
}

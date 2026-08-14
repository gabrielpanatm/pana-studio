use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use serde::Serialize;

use crate::source_graph::model::{
    BlockCapabilities, BlockDefinition, BlockOptionChoice, BlockOptionConstraints,
    BlockOptionControl, BlockOptionDefinition, BlockOptionValue, BlockOrigin, BlockRequirement,
    BlockRequirementKind, BlockScale, BlockSlotDefinition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeBlockKind {
    Js,
    Static,
}

impl NativeBlockKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Js => "js",
            Self::Static => "static",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeBlockDefinition {
    pub id: &'static str,
    pub schema_version: u32,
    pub family_id: &'static str,
    pub variant_id: &'static str,
    pub scale: BlockScale,
    pub kind: NativeBlockKind,
    pub label: &'static str,
    pub description: &'static str,
    pub tag: &'static str,
    pub class_name: &'static str,
    pub text: &'static str,
    pub html: &'static str,
    /// CSS structural indispensabil funcționării blocului. Nu conține temă,
    /// decor sau design implicit.
    pub functional_scss: &'static str,
    pub capabilities: BlockCapabilities,
    pub requirements: &'static [NativeBlockRequirement],
    pub options: &'static [NativeBlockOptionDefinition],
    pub slots: &'static [NativeBlockSlotDefinition],
}

#[derive(Clone, Copy, Debug)]
pub struct NativeBlockRequirement {
    pub id: &'static str,
    pub kind: BlockRequirementKind,
    pub minimum_version: u32,
    pub required: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeBlockSlotDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub required: bool,
    pub multiple: bool,
    pub item_kind: &'static str,
    pub minimum_items: usize,
    pub maximum_items: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub enum NativeBlockOptionDefault {
    Boolean(bool),
    Integer(i64),
    Text(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct NativeBlockOptionDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub control: BlockOptionControl,
    pub attribute: &'static str,
    pub default_value: NativeBlockOptionDefault,
    pub omit_when_default: bool,
    pub minimum: Option<i64>,
    pub maximum: Option<i64>,
    pub step: Option<i64>,
    pub maximum_length: Option<usize>,
    pub choices: &'static [(&'static str, &'static str)],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockRegistryItem {
    pub id: &'static str,
    pub schema_version: u32,
    pub family_id: &'static str,
    pub variant_id: &'static str,
    pub scale: BlockScale,
    pub kind: NativeBlockKind,
    pub label: &'static str,
    pub description: &'static str,
    pub tag: &'static str,
    pub text: &'static str,
    pub class_name: &'static str,
    pub html: &'static str,
    pub capabilities: BlockCapabilities,
    pub requirements: Vec<BlockRequirement>,
    pub options: Vec<BlockOptionDefinition>,
    pub slots: Vec<BlockSlotDefinition>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockRegistryGroup {
    pub label: &'static str,
    pub elements: Vec<NativeBlockRegistryItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockRegistrySnapshot {
    pub schema_version: u32,
    pub blocks: Vec<NativeBlockRegistryItem>,
    pub groups: Vec<NativeBlockRegistryGroup>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeBlockIdentity {
    pub class_name: String,
    pub data_anim: String,
    pub instance_id: String,
}

const NATIVE_JS_REQUIREMENTS: &[NativeBlockRequirement] = &[NativeBlockRequirement {
    id: "pana-block-runtime",
    kind: BlockRequirementKind::Runtime,
    minimum_version: 1,
    required: true,
}];

const NATIVE_JS_FUNCTIONAL_STYLE_REQUIREMENTS: &[NativeBlockRequirement] = &[
    NativeBlockRequirement {
        id: "pana-block-runtime",
        kind: BlockRequirementKind::Runtime,
        minimum_version: 1,
        required: true,
    },
    NativeBlockRequirement {
        id: "pana-block-functional-styles",
        kind: BlockRequirementKind::Stylesheet,
        minimum_version: 1,
        required: true,
    },
];

const NO_CHOICES: &[(&str, &str)] = &[];
const SIDE_CHOICES: &[(&str, &str)] = &[("start", "Început"), ("end", "Sfârșit")];
const NO_SLOTS: &[NativeBlockSlotDefinition] = &[];
const NO_REQUIREMENTS: &[NativeBlockRequirement] = &[];
const NO_OPTIONS: &[NativeBlockOptionDefinition] = &[];
const ACCORDION_SLOTS: &[NativeBlockSlotDefinition] = &[NativeBlockSlotDefinition {
    id: "items",
    label: "Elemente accordion",
    required: true,
    multiple: true,
    item_kind: "accordionItem",
    minimum_items: 1,
    maximum_items: Some(32),
}];
const TABS_SLOTS: &[NativeBlockSlotDefinition] = &[NativeBlockSlotDefinition {
    id: "items",
    label: "Taburi și panouri",
    required: true,
    multiple: true,
    item_kind: "tabItem",
    minimum_items: 1,
    maximum_items: Some(32),
}];
const SLIDER_SLOTS: &[NativeBlockSlotDefinition] = &[NativeBlockSlotDefinition {
    id: "slides",
    label: "Slide-uri",
    required: true,
    multiple: true,
    item_kind: "slide",
    minimum_items: 1,
    maximum_items: Some(32),
}];
const DIALOG_SLOTS: &[NativeBlockSlotDefinition] = &[
    NativeBlockSlotDefinition {
        id: "trigger",
        label: "Declanșator",
        required: true,
        multiple: false,
        item_kind: "trigger",
        minimum_items: 1,
        maximum_items: Some(1),
    },
    NativeBlockSlotDefinition {
        id: "content",
        label: "Conținut dialog",
        required: true,
        multiple: false,
        item_kind: "content",
        minimum_items: 1,
        maximum_items: Some(1),
    },
];
const OFFCANVAS_SLOTS: &[NativeBlockSlotDefinition] = &[
    NativeBlockSlotDefinition {
        id: "trigger",
        label: "Declanșator",
        required: true,
        multiple: false,
        item_kind: "trigger",
        minimum_items: 1,
        maximum_items: Some(1),
    },
    NativeBlockSlotDefinition {
        id: "content",
        label: "Conținut panou",
        required: true,
        multiple: false,
        item_kind: "content",
        minimum_items: 1,
        maximum_items: Some(1),
    },
];
const NAV_MENU_SLOTS: &[NativeBlockSlotDefinition] = &[NativeBlockSlotDefinition {
    id: "links",
    label: "Legături",
    required: true,
    multiple: true,
    item_kind: "link",
    minimum_items: 1,
    maximum_items: None,
}];

const COUNTER_OPTIONS: &[NativeBlockOptionDefinition] = &[
    NativeBlockOptionDefinition {
        id: "target",
        label: "Valoare finală",
        description: "Numărul la care se oprește animația.",
        control: BlockOptionControl::Number,
        attribute: "data-tinta",
        default_value: NativeBlockOptionDefault::Integer(1250),
        omit_when_default: false,
        minimum: Some(-1_000_000_000),
        maximum: Some(1_000_000_000),
        step: Some(1),
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "duration",
        label: "Durată",
        description: "Durata animației, în milisecunde.",
        control: BlockOptionControl::Number,
        attribute: "data-durata",
        default_value: NativeBlockOptionDefault::Integer(1800),
        omit_when_default: true,
        minimum: Some(100),
        maximum: Some(60_000),
        step: Some(50),
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "suffix",
        label: "Sufix",
        description: "Textul afișat după număr.",
        control: BlockOptionControl::Text,
        attribute: "data-sufix",
        default_value: NativeBlockOptionDefault::Text("+"),
        omit_when_default: false,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: Some(24),
        choices: NO_CHOICES,
    },
];

const ACCORDION_OPTIONS: &[NativeBlockOptionDefinition] = &[NativeBlockOptionDefinition {
    id: "allowMultiple",
    label: "Mai multe deschise",
    description: "Permite păstrarea simultană a mai multor secțiuni deschise.",
    control: BlockOptionControl::Toggle,
    attribute: "data-multiple",
    default_value: NativeBlockOptionDefault::Boolean(false),
    omit_when_default: true,
    minimum: None,
    maximum: None,
    step: None,
    maximum_length: None,
    choices: NO_CHOICES,
}];

const TABS_OPTIONS: &[NativeBlockOptionDefinition] = &[NativeBlockOptionDefinition {
    id: "initialTab",
    label: "Tab inițial",
    description: "Indexul tabului activ la încărcare, începând de la zero.",
    control: BlockOptionControl::Number,
    attribute: "data-default-tab",
    default_value: NativeBlockOptionDefault::Integer(0),
    omit_when_default: true,
    minimum: Some(0),
    maximum: Some(32),
    step: Some(1),
    maximum_length: None,
    choices: NO_CHOICES,
}];

const SLIDER_OPTIONS: &[NativeBlockOptionDefinition] = &[
    NativeBlockOptionDefinition {
        id: "accessibleLabel",
        label: "Etichetă accesibilă",
        description: "Numele sliderului pentru tehnologiile asistive.",
        control: BlockOptionControl::Text,
        attribute: "aria-label",
        default_value: NativeBlockOptionDefault::Text("Slider de continut"),
        omit_when_default: false,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: Some(120),
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "loop",
        label: "Navigare circulară",
        description: "Continuă de la ultimul slide la primul și invers.",
        control: BlockOptionControl::Toggle,
        attribute: "data-loop",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "initialSlide",
        label: "Slide inițial",
        description: "Indexul slide-ului inițial, începând de la zero.",
        control: BlockOptionControl::Number,
        attribute: "data-initial-slide",
        default_value: NativeBlockOptionDefault::Integer(0),
        omit_when_default: true,
        minimum: Some(0),
        maximum: Some(31),
        step: Some(1),
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "autoplay",
        label: "Redare automată",
        description: "Pornește rotația automată; implicit este oprită.",
        control: BlockOptionControl::Toggle,
        attribute: "data-autoplay",
        default_value: NativeBlockOptionDefault::Boolean(false),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "interval",
        label: "Interval autoplay",
        description: "Timpul dintre slide-uri, în milisecunde.",
        control: BlockOptionControl::Number,
        attribute: "data-interval",
        default_value: NativeBlockOptionDefault::Integer(5000),
        omit_when_default: true,
        minimum: Some(1000),
        maximum: Some(60_000),
        step: Some(100),
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "pauseOnHover",
        label: "Pauză la hover",
        description: "Oprește temporar autoplay când cursorul este peste slider.",
        control: BlockOptionControl::Toggle,
        attribute: "data-pause-hover",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "pauseOnFocus",
        label: "Pauză la focus",
        description: "Oprește temporar autoplay cât timp focusul este în slider.",
        control: BlockOptionControl::Toggle,
        attribute: "data-pause-focus",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "pauseOnInteraction",
        label: "Oprire la interacțiune",
        description: "Oprește autoplay după navigarea manuală.",
        control: BlockOptionControl::Toggle,
        attribute: "data-pause-interaction",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
];

const DIALOG_OPTIONS: &[NativeBlockOptionDefinition] = &[
    NativeBlockOptionDefinition {
        id: "closeOnBackdrop",
        label: "Închide la click exterior",
        description: "Închide dialogul când este apăsat fundalul.",
        control: BlockOptionControl::Toggle,
        attribute: "data-close-outside",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "closeOnEscape",
        label: "Închide cu Escape",
        description: "Permite închiderea dialogului de la tastatură.",
        control: BlockOptionControl::Toggle,
        attribute: "data-close-escape",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
];

const OFFCANVAS_OPTIONS: &[NativeBlockOptionDefinition] = &[
    NativeBlockOptionDefinition {
        id: "side",
        label: "Latură",
        description: "Latura din care intră panoul.",
        control: BlockOptionControl::Select,
        attribute: "data-pana-offcanvas-side",
        default_value: NativeBlockOptionDefault::Text("end"),
        omit_when_default: false,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: SIDE_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "closeOnBackdrop",
        label: "Închide la click exterior",
        description: "Închide panoul când este apăsat fundalul.",
        control: BlockOptionControl::Toggle,
        attribute: "data-close-outside",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "closeOnEscape",
        label: "Închide cu Escape",
        description: "Permite închiderea panoului de la tastatură.",
        control: BlockOptionControl::Toggle,
        attribute: "data-close-escape",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
];

const NAV_MENU_OPTIONS: &[NativeBlockOptionDefinition] = &[
    NativeBlockOptionDefinition {
        id: "accessibleLabel",
        label: "Etichetă accesibilă",
        description: "Numele meniului pentru tehnologiile asistive.",
        control: BlockOptionControl::Text,
        attribute: "aria-label",
        default_value: NativeBlockOptionDefault::Text("Navigatie principala"),
        omit_when_default: false,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: Some(120),
        choices: NO_CHOICES,
    },
    NativeBlockOptionDefinition {
        id: "closeOnSelect",
        label: "Închide după selectare",
        description: "Închide meniul mobil după activarea unei legături.",
        control: BlockOptionControl::Toggle,
        attribute: "data-close-on-select",
        default_value: NativeBlockOptionDefault::Boolean(true),
        omit_when_default: true,
        minimum: None,
        maximum: None,
        step: None,
        maximum_length: None,
        choices: NO_CHOICES,
    },
];

const NATIVE_BLOCKS: &[NativeBlockDefinition] = &[
    NativeBlockDefinition {
        id: "icon",
        schema_version: 1,
        family_id: "graphics",
        variant_id: "tabler-outline",
        scale: BlockScale::Element,
        kind: NativeBlockKind::Static,
        label: "Icon",
        description: "icon SVG inline din catalogul Tabler Outline administrat de Rust",
        tag: "svg",
        text: "",
        class_name: "icon",
        // Geometria nu este publicată drept payload HTML în frontend. Rendererul
        // Rust o construiește din registrul compilat pentru fiecare inserare.
        html: "",
        functional_scss: "",
        capabilities: BlockCapabilities {
            can_insert: true,
            can_edit_properties: true,
            supports_variants: false,
            supports_slots: false,
        },
        requirements: NO_REQUIREMENTS,
        options: NO_OPTIONS,
        slots: NO_SLOTS,
    },
    NativeBlockDefinition {
        id: "counter",
        schema_version: 1,
        family_id: "data-display",
        variant_id: "default",
        scale: BlockScale::Element,
        kind: NativeBlockKind::Js,
        label: "Counter",
        description: "numar animat la scroll",
        tag: "span",
        text: "0",
        class_name: "counter",
        html: r#"<span class="counter __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-block="counter" data-pana-instance="__PANA_INSTANCE__" data-tinta="1250" data-sufix="+">0</span>"#,
        functional_scss: "",
        capabilities: BlockCapabilities {
            can_insert: true,
            can_edit_properties: true,
            supports_variants: false,
            supports_slots: false,
        },
        requirements: NATIVE_JS_REQUIREMENTS,
        options: COUNTER_OPTIONS,
        slots: NO_SLOTS,
    },
    NativeBlockDefinition {
        id: "accordion",
        schema_version: 1,
        family_id: "disclosure",
        variant_id: "default",
        scale: BlockScale::Composition,
        kind: NativeBlockKind::Js,
        label: "Accordion",
        description: "sectiuni expandabile",
        tag: "div",
        text: "",
        class_name: "accordion",
        html: r#"<div class="accordion __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-block="accordion" data-pana-instance="__PANA_INSTANCE__">
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
        functional_scss: "",
        capabilities: BlockCapabilities {
            can_insert: true,
            can_edit_properties: true,
            supports_variants: false,
            supports_slots: true,
        },
        requirements: NATIVE_JS_REQUIREMENTS,
        options: ACCORDION_OPTIONS,
        slots: ACCORDION_SLOTS,
    },
    NativeBlockDefinition {
        id: "tabs",
        schema_version: 1,
        family_id: "navigation",
        variant_id: "default",
        scale: BlockScale::Composition,
        kind: NativeBlockKind::Js,
        label: "Tabs",
        description: "panouri comutabile",
        tag: "div",
        text: "",
        class_name: "tabs",
        html: r#"<div class="tabs __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-block="tabs" data-pana-instance="__PANA_INSTANCE__">
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
        functional_scss: "",
        capabilities: BlockCapabilities {
            can_insert: true,
            can_edit_properties: true,
            supports_variants: false,
            supports_slots: true,
        },
        requirements: NATIVE_JS_REQUIREMENTS,
        options: TABS_OPTIONS,
        slots: TABS_SLOTS,
    },
    NativeBlockDefinition {
        id: "slider",
        schema_version: 1,
        family_id: "content-carousel",
        variant_id: "single",
        scale: BlockScale::Composition,
        kind: NativeBlockKind::Js,
        label: "Slider",
        description: "carousel de continut integrabil in orice container compatibil",
        tag: "div",
        text: "",
        class_name: "slider",
        html: r#"<div class="slider __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-block="slider" data-pana-instance="__PANA_INSTANCE__" role="group" aria-roledescription="carousel" aria-label="Slider de continut" tabindex="0">
  <div class="slider__viewport" data-pana-slider-viewport>
    <div class="slider__track" data-pana-slider-track data-pana-slot="slides" aria-live="polite">
      <div class="slider__slide" data-pana-slider-slide role="group" aria-roledescription="slide">
        <h3>Primul slide</h3>
        <p>Editeaza continutul direct in Canvas.</p>
      </div>
      <div class="slider__slide" data-pana-slider-slide role="group" aria-roledescription="slide">
        <h3>Al doilea slide</h3>
        <p>Adauga aici orice continut potrivit sectiunii tale.</p>
      </div>
    </div>
  </div>
  <div class="slider__controls" data-pana-slider-controls>
    <button class="slider__button slider__previous" data-pana-slider-previous type="button" aria-label="Slide-ul anterior">Anterior</button>
    <div class="slider__indicators" data-pana-slider-indicators aria-label="Alege slide-ul"></div>
    <button class="slider__button slider__next" data-pana-slider-next type="button" aria-label="Slide-ul urmator">Urmator</button>
    <button class="slider__button slider__autoplay" data-pana-slider-autoplay type="button" aria-label="Opreste rotatia" hidden>Opreste</button>
  </div>
</div>"#,
        functional_scss: "",
        capabilities: BlockCapabilities {
            can_insert: true,
            can_edit_properties: true,
            supports_variants: false,
            supports_slots: true,
        },
        requirements: NATIVE_JS_REQUIREMENTS,
        options: SLIDER_OPTIONS,
        slots: SLIDER_SLOTS,
    },
    NativeBlockDefinition {
        id: "dialog",
        schema_version: 1,
        family_id: "overlay",
        variant_id: "modal",
        scale: BlockScale::Composition,
        kind: NativeBlockKind::Js,
        label: "Dialog",
        description: "dialog cu trigger",
        tag: "div",
        text: "",
        class_name: "dialog",
        html: r#"<div class="dialog __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-block="dialog" data-pana-instance="__PANA_INSTANCE__">
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
        functional_scss: r#".dialog__overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: grid;
  place-items: center;
  overflow: auto;
}

.dialog__overlay[hidden] {
  display: none;
}

.dialog__panel {
  max-width: 100%;
  max-height: 100%;
  overflow: auto;
  background: Canvas;
  color: CanvasText;
}"#,
        capabilities: BlockCapabilities {
            can_insert: true,
            can_edit_properties: true,
            supports_variants: true,
            supports_slots: true,
        },
        requirements: NATIVE_JS_FUNCTIONAL_STYLE_REQUIREMENTS,
        options: DIALOG_OPTIONS,
        slots: DIALOG_SLOTS,
    },
    NativeBlockDefinition {
        id: "offcanvas",
        schema_version: 1,
        family_id: "overlay",
        variant_id: "drawer",
        scale: BlockScale::Composition,
        kind: NativeBlockKind::Js,
        label: "Offcanvas",
        description: "panou lateral glisant",
        tag: "div",
        text: "",
        class_name: "offcanvas",
        html: r#"<div class="offcanvas __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-block="offcanvas" data-pana-instance="__PANA_INSTANCE__" data-pana-offcanvas-side="end">
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
        functional_scss: r#".offcanvas__overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  justify-content: flex-end;
  overflow: hidden;
}

.offcanvas__overlay[hidden] {
  display: none;
}

.offcanvas__panel {
  width: min(28rem, 100%);
  height: 100%;
  overflow: auto;
  background: Canvas;
  color: CanvasText;
}

.offcanvas[data-pana-offcanvas-side="start"] .offcanvas__overlay {
  justify-content: flex-start;
}

"#,
        capabilities: BlockCapabilities {
            can_insert: true,
            can_edit_properties: true,
            supports_variants: true,
            supports_slots: true,
        },
        requirements: NATIVE_JS_FUNCTIONAL_STYLE_REQUIREMENTS,
        options: OFFCANVAS_OPTIONS,
        slots: OFFCANVAS_SLOTS,
    },
    NativeBlockDefinition {
        id: "nav-menu",
        schema_version: 1,
        family_id: "navigation",
        variant_id: "responsive",
        scale: BlockScale::Composition,
        kind: NativeBlockKind::Js,
        label: "Meniu navigatie",
        description: "meniu responsive cu toggle",
        tag: "nav",
        text: "",
        class_name: "nav-menu",
        html: r#"<nav class="nav-menu __PANA_CLASS__" data-anim="__PANA_DATA_ANIM__" data-pana-block="nav-menu" data-pana-instance="__PANA_INSTANCE__" aria-label="Navigatie principala">
  <div class="nav-menu__bar">
    <a class="nav-menu__brand" href="/">Pana Site</a>
    <button class="nav-menu__toggle" data-pana-nav-menu-toggle type="button" aria-expanded="false" hidden>
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
        functional_scss: "",
        capabilities: BlockCapabilities {
            can_insert: true,
            can_edit_properties: true,
            supports_variants: true,
            supports_slots: true,
        },
        requirements: NATIVE_JS_REQUIREMENTS,
        options: NAV_MENU_OPTIONS,
        slots: NAV_MENU_SLOTS,
    },
];

pub(crate) fn native_block_provider_definitions() -> &'static [NativeBlockDefinition] {
    NATIVE_BLOCKS
}

pub fn native_block_by_id(id: &str) -> Option<&'static NativeBlockDefinition> {
    let normalized = id.trim();
    NATIVE_BLOCKS.iter().find(|block| block.id == normalized)
}

pub fn known_native_block_ids() -> impl Iterator<Item = &'static str> {
    NATIVE_BLOCKS.iter().map(|block| block.id)
}

pub fn native_block_registry_snapshot() -> NativeBlockRegistrySnapshot {
    let blocks = NATIVE_BLOCKS
        .iter()
        .map(NativeBlockRegistryItem::from_definition)
        .collect::<Vec<_>>();
    let js_blocks = blocks
        .iter()
        .filter(|block| block.kind == NativeBlockKind::Js)
        .cloned()
        .collect::<Vec<_>>();
    let static_blocks = blocks
        .iter()
        .filter(|block| block.kind == NativeBlockKind::Static)
        .cloned()
        .collect::<Vec<_>>();

    NativeBlockRegistrySnapshot {
        schema_version: 1,
        blocks,
        groups: vec![
            NativeBlockRegistryGroup {
                label: "Elements",
                elements: static_blocks,
            },
            NativeBlockRegistryGroup {
                label: "Interactive",
                elements: js_blocks,
            },
        ],
    }
}

pub(crate) fn native_block_contract_definition(block: &NativeBlockDefinition) -> BlockDefinition {
    BlockDefinition {
        id: format!("native/{}", block.id),
        schema_version: block.schema_version,
        provider_id: block.id.to_string(),
        family_id: block.family_id.to_string(),
        variant_id: block.variant_id.to_string(),
        display_name: block.label.to_string(),
        description: block.description.to_string(),
        origin: BlockOrigin::Native,
        scale: block.scale,
        capabilities: block.capabilities,
        requirements: block
            .requirements
            .iter()
            .map(BlockRequirement::from)
            .collect(),
        options: block
            .options
            .iter()
            .map(BlockOptionDefinition::from)
            .collect(),
        slots: block.slots.iter().map(BlockSlotDefinition::from).collect(),
    }
}

pub fn native_block_preview_css<'a>(ids: impl IntoIterator<Item = &'a str>) -> String {
    ids.into_iter()
        .filter_map(native_block_by_id)
        .map(|block| block.functional_scss.trim())
        .filter(|scss| !scss.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn render_native_block_html(
    block: &NativeBlockDefinition,
    identity: &NativeBlockIdentity,
) -> String {
    if block.kind == NativeBlockKind::Static && block.id == "icon" {
        return super::icons::render_icon_block_html(
            super::icons::DEFAULT_ICON_ID,
            &identity.class_name,
            &identity.data_anim,
            &identity.instance_id,
        )
        .expect("registrul Tabler Outline compilat trebuie validat la testare și build");
    }
    block
        .html
        .replace("__PANA_CLASS__", &identity.class_name)
        .replace("__PANA_DATA_ANIM__", &identity.data_anim)
        .replace("__PANA_INSTANCE__", &identity.instance_id)
}

pub fn native_block_root_class_name(
    block: &NativeBlockDefinition,
    identity: &NativeBlockIdentity,
) -> String {
    [block.class_name, identity.class_name.as_str()]
        .into_iter()
        .filter(|token| !token.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn unique_native_block_identity<F>(
    block_id: &str,
    seed: &str,
    mut exists: F,
) -> NativeBlockIdentity
where
    F: FnMut(&str) -> bool,
{
    let block_token = normalize_block_token(block_id);
    for attempt in 0..80u32 {
        let token = identity_token(seed, attempt);
        let candidate = format!("ps-{block_token}-{token}");
        if !exists(&candidate) {
            return NativeBlockIdentity {
                class_name: candidate.clone(),
                data_anim: candidate.clone(),
                instance_id: native_block_instance_id(block_id, &candidate),
            };
        }
    }

    let fallback = format!("ps-{block_token}-{}", identity_token(seed, 80));
    NativeBlockIdentity {
        class_name: fallback.clone(),
        data_anim: fallback.clone(),
        instance_id: native_block_instance_id(block_id, &fallback),
    }
}

pub(crate) fn native_block_instance_id(block_id: &str, unique_token: &str) -> String {
    let trimmed = unique_token
        .strip_prefix("ps-")
        .unwrap_or(unique_token)
        .trim();
    format!("{}-{}", block_id.trim(), trimmed)
}

fn normalize_block_token(value: &str) -> String {
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
        "block".to_string()
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

impl NativeBlockRegistryItem {
    fn from_definition(block: &NativeBlockDefinition) -> Self {
        Self {
            id: block.id,
            schema_version: block.schema_version,
            family_id: block.family_id,
            variant_id: block.variant_id,
            scale: block.scale,
            kind: block.kind,
            label: block.label,
            description: block.description,
            tag: block.tag,
            text: block.text,
            class_name: block.class_name,
            html: block.html,
            capabilities: block.capabilities,
            requirements: block
                .requirements
                .iter()
                .map(BlockRequirement::from)
                .collect(),
            options: block
                .options
                .iter()
                .map(BlockOptionDefinition::from)
                .collect(),
            slots: block.slots.iter().map(BlockSlotDefinition::from).collect(),
        }
    }
}

impl From<&NativeBlockRequirement> for BlockRequirement {
    fn from(requirement: &NativeBlockRequirement) -> Self {
        Self {
            id: requirement.id.to_string(),
            kind: requirement.kind,
            minimum_version: requirement.minimum_version,
            required: requirement.required,
        }
    }
}

impl From<&NativeBlockSlotDefinition> for BlockSlotDefinition {
    fn from(slot: &NativeBlockSlotDefinition) -> Self {
        Self {
            id: slot.id.to_string(),
            label: slot.label.to_string(),
            required: slot.required,
            multiple: slot.multiple,
            item_kind: slot.item_kind.to_string(),
            minimum_items: slot.minimum_items,
            maximum_items: slot.maximum_items,
        }
    }
}

impl From<&NativeBlockOptionDefinition> for BlockOptionDefinition {
    fn from(option: &NativeBlockOptionDefinition) -> Self {
        Self {
            id: option.id.to_string(),
            label: option.label.to_string(),
            description: option.description.to_string(),
            control: option.control,
            attribute: option.attribute.to_string(),
            default_value: option.default_value.into(),
            omit_when_default: option.omit_when_default,
            constraints: BlockOptionConstraints {
                minimum: option.minimum,
                maximum: option.maximum,
                step: option.step,
                maximum_length: option.maximum_length,
            },
            choices: option
                .choices
                .iter()
                .map(|(value, label)| BlockOptionChoice {
                    value: (*value).to_string(),
                    label: (*label).to_string(),
                })
                .collect(),
        }
    }
}

impl From<NativeBlockOptionDefault> for BlockOptionValue {
    fn from(value: NativeBlockOptionDefault) -> Self {
        match value {
            NativeBlockOptionDefault::Boolean(value) => Self::Boolean(value),
            NativeBlockOptionDefault::Integer(value) => Self::Integer(value),
            NativeBlockOptionDefault::Text(value) => Self::Text(value.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_block_html_with_kernel_identity() {
        let block = native_block_by_id("counter").unwrap();
        let identity = NativeBlockIdentity {
            class_name: "ps-counter-12345678".to_string(),
            data_anim: "ps-counter-12345678".to_string(),
            instance_id: "counter-counter-12345678".to_string(),
        };

        let html = render_native_block_html(block, &identity);

        assert!(html.contains(r#"data-pana-block="counter""#));
        assert!(html.contains(r#"class="counter ps-counter-12345678""#));
        assert!(html.contains(r#"data-pana-instance="counter-counter-12345678""#));
        assert!(!html.contains("__PANA_"));
    }

    #[test]
    fn generated_identity_skips_collisions() {
        let identity = unique_native_block_identity("nav-menu", "seed", |candidate| {
            candidate.ends_with("38d91c63")
        });

        assert!(identity.class_name.starts_with("ps-nav-menu-"));
        assert_eq!(identity.class_name, identity.data_anim);
        assert!(identity.instance_id.starts_with("nav-menu-nav-menu-"));
    }

    #[test]
    fn registry_snapshot_exports_html_palette_contract() {
        let snapshot = native_block_registry_snapshot();
        let serialized_kind = serde_json::to_string(&NativeBlockKind::Js).unwrap();

        assert_eq!(serialized_kind, "\"js\"");
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.blocks.len(), NATIVE_BLOCKS.len());
        assert_eq!(snapshot.groups.len(), 2);
        assert_eq!(snapshot.groups[0].label, "Elements");
        assert_eq!(snapshot.groups[0].elements.len(), 1);
        assert_eq!(snapshot.groups[1].label, "Interactive");
        assert_eq!(snapshot.groups[1].elements.len(), NATIVE_BLOCKS.len() - 1);

        for block in snapshot.blocks {
            assert!(!block.id.trim().is_empty());
            assert!(!block.label.trim().is_empty());
            assert!(!block.description.trim().is_empty());
            if block.kind == NativeBlockKind::Static {
                assert!(block.html.is_empty());
            } else {
                assert!(block.html.contains("data-pana-block="));
                assert!(block.html.contains("__PANA_CLASS__"));
            }
            assert_eq!(native_block_by_id(block.id).unwrap().tag, block.tag);
        }
    }

    #[test]
    fn registry_owns_unique_typed_option_contracts() {
        for block in NATIVE_BLOCKS {
            let mut ids = std::collections::HashSet::new();
            let mut attributes = std::collections::HashSet::new();
            for option in block.options {
                assert!(ids.insert(option.id), "opțiune duplicată: {}", option.id);
                assert!(
                    attributes.insert(option.attribute),
                    "atribut duplicat: {}",
                    option.attribute
                );
                if !option.omit_when_default {
                    assert!(
                        block.html.contains(option.attribute),
                        "{} trebuie să emită default-ul canonic {}",
                        block.id,
                        option.attribute
                    );
                }
                if option.control == BlockOptionControl::Select {
                    assert!(!option.choices.is_empty());
                }
            }
        }
    }

    #[test]
    fn current_registry_reserves_section_scale_for_page_level_blocks() {
        for provider_id in ["icon", "counter"] {
            assert_eq!(
                native_block_by_id(provider_id).unwrap().scale,
                BlockScale::Element,
                "{provider_id} trebuie să rămână element atomic"
            );
        }
        for provider_id in [
            "accordion",
            "tabs",
            "slider",
            "dialog",
            "offcanvas",
            "nav-menu",
        ] {
            assert_eq!(
                native_block_by_id(provider_id).unwrap().scale,
                BlockScale::Composition,
                "{provider_id} trebuie să fie compoziție integrabilă"
            );
        }
        assert!(NATIVE_BLOCKS
            .iter()
            .all(|block| block.scale != BlockScale::Section));
    }

    #[test]
    fn slider_is_an_embeddable_rust_owned_composition() {
        let slider = native_block_by_id("slider").expect("slider provider");
        assert_eq!(slider.kind, NativeBlockKind::Js);
        assert_eq!(slider.scale, BlockScale::Composition);
        assert_eq!(slider.tag, "div");
        assert!(slider.capabilities.supports_slots);
        assert_eq!(slider.slots[0].id, "slides");
        assert_eq!(slider.slots[0].minimum_items, 1);
        assert_eq!(slider.slots[0].maximum_items, Some(32));
        assert!(slider.html.contains("data-pana-slot=\"slides\""));
        assert!(slider.html.contains("aria-roledescription=\"carousel\""));
        assert!(slider.html.contains("tabindex=\"0\""));
        assert!(!slider.html.contains("data-autoplay=\"true\""));
        assert!(slider.functional_scss.is_empty());
    }

    #[test]
    fn native_styles_are_functional_only() {
        for block in NATIVE_BLOCKS {
            assert!(!block.functional_scss.contains("--pana-block-"));
            assert!(!block.functional_scss.contains("box-shadow"));
            assert!(!block.functional_scss.contains("border-radius"));
            assert!(!block.functional_scss.contains("transition"));
        }

        for id in ["icon", "counter", "accordion", "tabs", "slider", "nav-menu"] {
            assert!(native_block_by_id(id).unwrap().functional_scss.is_empty());
        }
        for id in ["dialog", "offcanvas"] {
            let block = native_block_by_id(id).unwrap();
            assert!(!block.functional_scss.is_empty());
            assert!(block
                .requirements
                .iter()
                .any(|requirement| requirement.kind == BlockRequirementKind::Stylesheet));
        }
    }
}

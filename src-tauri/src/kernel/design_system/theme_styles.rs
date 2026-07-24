use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    css::{
        rules::{get_exact_rule_properties, update_exact_css_rule},
        validation::validate_panel_rule_input,
        variables::parse_variables_from_source,
    },
    kernel::file_buffer_store::FileBufferStore,
    zola_theme::active_theme_from_source,
};

pub const THEME_STYLE_CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeStyleControlKind {
    Text,
    Color,
    Choice,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStyleControlOption {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStylePropertySnapshot {
    pub id: String,
    pub label: String,
    pub control: ThemeStyleControlKind,
    pub options: Vec<ThemeStyleControlOption>,
    pub value: Option<String>,
    pub effective_value: Option<String>,
    pub inherited_from: Option<String>,
    pub token_name: Option<String>,
    pub can_clear: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStyleTargetSnapshot {
    pub id: String,
    pub category_id: String,
    pub label: String,
    pub description: String,
    pub selector: String,
    pub parent_id: Option<String>,
    pub preview_kind: String,
    pub sample_text: String,
    pub source_path: String,
    pub editable: bool,
    pub diagnostic: Option<String>,
    pub has_overrides: bool,
    pub properties: Vec<ThemeStylePropertySnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStyleCategorySnapshot {
    pub id: String,
    pub label: String,
    pub target_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStyleCatalogSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub source_path: String,
    pub source_origin: String,
    pub categories: Vec<ThemeStyleCategorySnapshot>,
    pub targets: Vec<ThemeStyleTargetSnapshot>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStylePropertyInput {
    pub id: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStylePreviewProperty {
    pub id: String,
    pub value: String,
    pub inherited: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStyleDraftPreview {
    pub schema_version: u32,
    pub target_id: String,
    pub selector: String,
    pub source_path: String,
    pub css: String,
    pub properties: Vec<ThemeStylePreviewProperty>,
}

#[derive(Clone)]
struct PropertyDefinition {
    id: &'static str,
    label: &'static str,
    control: ThemeStyleControlKind,
    options: Vec<(&'static str, &'static str)>,
    can_clear: bool,
}

#[derive(Clone)]
struct TargetDefinition {
    id: &'static str,
    category_id: &'static str,
    label: &'static str,
    description: &'static str,
    selector: &'static str,
    parent_id: Option<&'static str>,
    preview_kind: &'static str,
    sample_text: &'static str,
    properties: Vec<PropertyDefinition>,
}

fn property(
    id: &'static str,
    label: &'static str,
    control: ThemeStyleControlKind,
    can_clear: bool,
) -> PropertyDefinition {
    PropertyDefinition {
        id,
        label,
        control,
        options: Vec::new(),
        can_clear,
    }
}

fn choice(
    id: &'static str,
    label: &'static str,
    can_clear: bool,
    options: &[(&'static str, &'static str)],
) -> PropertyDefinition {
    PropertyDefinition {
        id,
        label,
        control: ThemeStyleControlKind::Choice,
        options: options.to_vec(),
        can_clear,
    }
}

fn target(
    id: &'static str,
    category_id: &'static str,
    label: &'static str,
    description: &'static str,
    selector: &'static str,
    parent_id: Option<&'static str>,
    preview_kind: &'static str,
    sample_text: &'static str,
    properties: Vec<PropertyDefinition>,
) -> TargetDefinition {
    TargetDefinition {
        id,
        category_id,
        label,
        description,
        selector,
        parent_id,
        preview_kind,
        sample_text,
        properties,
    }
}

fn style_categories() -> Vec<(&'static str, &'static str)> {
    vec![
        ("general", "General"),
        ("typography", "Tipografie"),
        ("links", "Linkuri"),
        ("media", "Media"),
        ("lists", "Liste"),
        ("quotes-code", "Citate și cod"),
        ("tables", "Tabele"),
        ("forms", "Formulare"),
        ("auxiliary", "Elemente auxiliare"),
    ]
}

fn style_registry() -> Vec<TargetDefinition> {
    use ThemeStyleControlKind::{Color, Text};

    let font_style = [("normal", "Normal"), ("italic", "Cursiv")];
    let text_align = [
        ("left", "Stânga"),
        ("center", "Centru"),
        ("right", "Dreapta"),
    ];
    let text_wrap = [
        ("wrap", "Normal"),
        ("balance", "Echilibrat"),
        ("pretty", "Tipografic"),
        ("nowrap", "Fără rupere"),
    ];
    let decoration = [("none", "Fără"), ("underline", "Subliniat")];

    vec![
        target(
            "general.body",
            "general",
            "Pagină",
            "Aspectul implicit al documentului și fundalul paginii.",
            "body",
            None,
            "body",
            "Textul de bază al paginii",
            vec![
                property("font-family", "Familie font", Text, false),
                property("font-size", "Mărime text", Text, false),
                property("font-weight", "Greutate", Text, false),
                property("line-height", "Înălțime rând", Text, false),
                property("color", "Culoare text", Color, false),
                property("background-color", "Culoare fundal", Color, false),
            ],
        ),
        target(
            "typography.headings",
            "typography",
            "Toate titlurile",
            "Baza comună moștenită de titlurile H1–H6.",
            "h1, h2, h3, h4, h5, h6",
            None,
            "heading",
            "Titlu de pagină echilibrat",
            vec![
                property("font-family", "Familie font", Text, false),
                property("font-weight", "Greutate", Text, false),
                property("line-height", "Înălțime rând", Text, false),
                property("letter-spacing", "Spațiere litere", Text, false),
                property("color", "Culoare", Color, false),
                choice("text-wrap", "Rupere text", false, &text_wrap),
            ],
        ),
        target(
            "typography.h1",
            "typography",
            "Titlu H1",
            "Titlul principal al paginii.",
            "h1",
            Some("typography.headings"),
            "h1",
            "Titlul principal al paginii",
            vec![
                property("font-size", "Mărime", Text, false),
                property("font-family", "Familie font", Text, true),
                property("font-weight", "Greutate", Text, true),
                property("line-height", "Înălțime rând", Text, true),
                property("letter-spacing", "Spațiere litere", Text, true),
                property("color", "Culoare", Color, true),
                choice("text-align", "Aliniere", true, &text_align),
            ],
        ),
        target(
            "typography.h2",
            "typography",
            "Titlu H2",
            "Titlul principal al unei secțiuni.",
            "h2",
            Some("typography.headings"),
            "h2",
            "Titlu principal de secțiune",
            vec![
                property("font-size", "Mărime", Text, false),
                property("font-family", "Familie font", Text, true),
                property("font-weight", "Greutate", Text, true),
                property("line-height", "Înălțime rând", Text, true),
                property("letter-spacing", "Spațiere litere", Text, true),
                property("color", "Culoare", Color, true),
                choice("text-align", "Aliniere", true, &text_align),
            ],
        ),
        target(
            "typography.h3",
            "typography",
            "Titlu H3",
            "Subtitlu de secțiune.",
            "h3",
            Some("typography.headings"),
            "h3",
            "Subtitlu de secțiune",
            vec![
                property("font-size", "Mărime", Text, false),
                property("font-family", "Familie font", Text, true),
                property("font-weight", "Greutate", Text, true),
                property("line-height", "Înălțime rând", Text, true),
                property("letter-spacing", "Spațiere litere", Text, true),
                property("color", "Culoare", Color, true),
            ],
        ),
        target(
            "typography.h4",
            "typography",
            "Titlu H4",
            "Titlu de nivel patru.",
            "h4",
            Some("typography.headings"),
            "h4",
            "Titlu de nivel patru",
            vec![
                property("font-size", "Mărime", Text, false),
                property("font-family", "Familie font", Text, true),
                property("font-weight", "Greutate", Text, true),
                property("line-height", "Înălțime rând", Text, true),
                property("letter-spacing", "Spațiere litere", Text, true),
                property("color", "Culoare", Color, true),
            ],
        ),
        target(
            "typography.h5",
            "typography",
            "Titlu H5",
            "Titlu de nivel cinci.",
            "h5",
            Some("typography.headings"),
            "h5",
            "Titlu de nivel cinci",
            vec![
                property("font-size", "Mărime", Text, false),
                property("font-family", "Familie font", Text, true),
                property("font-weight", "Greutate", Text, true),
                property("line-height", "Înălțime rând", Text, true),
                property("letter-spacing", "Spațiere litere", Text, true),
                property("color", "Culoare", Color, true),
            ],
        ),
        target(
            "typography.h6",
            "typography",
            "Titlu H6",
            "Titlu de nivel șase.",
            "h6",
            Some("typography.headings"),
            "h6",
            "Titlu de nivel șase",
            vec![
                property("font-size", "Mărime", Text, false),
                property("font-family", "Familie font", Text, true),
                property("font-weight", "Greutate", Text, true),
                property("line-height", "Înălțime rând", Text, true),
                property("letter-spacing", "Spațiere litere", Text, true),
                property("color", "Culoare", Color, true),
            ],
        ),
        target(
            "typography.paragraph",
            "typography",
            "Paragraf",
            "Textul curent din pagini și articole.",
            "p",
            Some("general.body"),
            "paragraph",
            "Un paragraf clar și confortabil de citit, folosit pentru conținutul curent.",
            vec![
                property("font-size", "Mărime", Text, false),
                property("line-height", "Înălțime rând", Text, false),
                property("color", "Culoare", Color, false),
                property("max-width", "Lățime maximă", Text, false),
                choice("text-wrap", "Rupere text", false, &text_wrap),
            ],
        ),
        target(
            "typography.lead",
            "typography",
            "Paragraf introductiv",
            "Text introductiv cu accent vizual.",
            "p.lead",
            Some("typography.paragraph"),
            "lead",
            "O introducere care stabilește tonul paginii.",
            vec![
                property("font-size", "Mărime", Text, true),
                property("line-height", "Înălțime rând", Text, true),
                property("color", "Culoare", Color, true),
                property("max-width", "Lățime maximă", Text, true),
            ],
        ),
        target(
            "typography.strong",
            "typography",
            "Text evidențiat",
            "Text marcat semantic cu strong sau bold.",
            "b, strong",
            Some("general.body"),
            "inline",
            "Text important",
            vec![
                property("font-weight", "Greutate", Text, false),
                property("color", "Culoare", Color, false),
            ],
        ),
        target(
            "typography.emphasis",
            "typography",
            "Text accentuat",
            "Accent tipografic semantic.",
            "em, i",
            Some("general.body"),
            "inline",
            "Text accentuat",
            vec![choice("font-style", "Stil", false, &font_style)],
        ),
        target(
            "typography.small",
            "typography",
            "Text secundar",
            "Text de dimensiune redusă pentru explicații.",
            "small",
            Some("general.body"),
            "small",
            "Informație secundară",
            vec![
                property("font-size", "Mărime", Text, false),
                property("color", "Culoare", Color, false),
            ],
        ),
        target(
            "typography.mark",
            "typography",
            "Text marcat",
            "Marcaj vizual în interiorul textului.",
            "mark",
            Some("general.body"),
            "mark",
            "Fragment evidențiat",
            vec![
                property("background-color", "Fundal", Color, false),
                property("color", "Culoare", Color, false),
                property("padding-inline", "Spațiere laterală", Text, false),
                property("border-radius", "Rotunjire", Text, false),
            ],
        ),
        target(
            "links.base",
            "links",
            "Link implicit",
            "Comportamentul vizual de bază al linkurilor.",
            "a",
            None,
            "link",
            "Link implicit",
            vec![
                property("color", "Culoare", Color, false),
                choice("text-decoration", "Decorație", false, &decoration),
            ],
        ),
        target(
            "links.content",
            "links",
            "Link în conținut",
            "Linkurile din articole și zonele de conținut.",
            "article a, .content a",
            Some("links.base"),
            "link",
            "Citește documentația completă",
            vec![
                property("color", "Culoare", Color, true),
                choice("text-decoration", "Decorație", true, &decoration),
                property("text-underline-offset", "Distanță subliniere", Text, true),
                property(
                    "text-decoration-thickness",
                    "Grosime subliniere",
                    Text,
                    true,
                ),
            ],
        ),
        target(
            "links.content-hover",
            "links",
            "Link la trecerea cursorului",
            "Starea hover a linkurilor din conținut.",
            "article a:hover, .content a:hover",
            Some("links.content"),
            "link-hover",
            "Link activ la trecerea cursorului",
            vec![
                property("color", "Culoare", Color, true),
                property(
                    "text-decoration-thickness",
                    "Grosime subliniere",
                    Text,
                    true,
                ),
            ],
        ),
        target(
            "media.image",
            "media",
            "Imagine",
            "Prezentarea implicită a imaginilor.",
            "img",
            None,
            "image",
            "Previzualizare imagine",
            vec![
                property("border-radius", "Rotunjire", Text, false),
                choice(
                    "object-fit",
                    "Încadrare",
                    true,
                    &[
                        ("cover", "Acoperă"),
                        ("contain", "Conține"),
                        ("fill", "Umple"),
                        ("none", "Natural"),
                    ],
                ),
                property("aspect-ratio", "Raport", Text, true),
            ],
        ),
        target(
            "media.figure",
            "media",
            "Figură",
            "Containerul semantic pentru imagine și legendă.",
            "figure",
            None,
            "figure",
            "Figură cu legendă",
            vec![
                property("margin-inline", "Margine laterală", Text, false),
                property("max-width", "Lățime maximă", Text, true),
            ],
        ),
        target(
            "media.caption",
            "media",
            "Legendă imagine",
            "Textul descriptiv de sub o figură.",
            "figcaption",
            Some("general.body"),
            "caption",
            "Legendă explicativă pentru imagine",
            vec![
                property("margin-top", "Distanță sus", Text, false),
                property("font-size", "Mărime", Text, false),
                property("color", "Culoare", Color, false),
                choice("text-align", "Aliniere", false, &text_align),
            ],
        ),
        target(
            "media.svg",
            "media",
            "Pictogramă SVG",
            "Culoarea implicită a graficii SVG.",
            "svg",
            None,
            "svg",
            "Pictogramă",
            vec![property("fill", "Umplere", Color, false)],
        ),
        target(
            "lists.content",
            "lists",
            "Liste în conținut",
            "Spațierea comună a listelor ordonate și neordonate.",
            "article ul, article ol, .content ul, .content ol",
            None,
            "list",
            "Primul element|Al doilea element|Al treilea element",
            vec![
                property("padding-left", "Indentare", Text, false),
                property("gap", "Spațiere elemente", Text, false),
            ],
        ),
        target(
            "lists.unordered",
            "lists",
            "Listă neordonată",
            "Marcajul listelor cu puncte.",
            "article ul, .content ul",
            Some("lists.content"),
            "unordered-list",
            "Primul element|Al doilea element|Al treilea element",
            vec![choice(
                "list-style",
                "Marcaj",
                false,
                &[("disc", "Disc"), ("circle", "Cerc"), ("square", "Pătrat")],
            )],
        ),
        target(
            "lists.ordered",
            "lists",
            "Listă ordonată",
            "Numerotarea listelor ordonate.",
            "article ol, .content ol",
            Some("lists.content"),
            "ordered-list",
            "Primul pas|Al doilea pas|Al treilea pas",
            vec![choice(
                "list-style",
                "Numerotare",
                false,
                &[
                    ("decimal", "Cifre"),
                    ("lower-alpha", "Litere mici"),
                    ("upper-roman", "Cifre romane"),
                ],
            )],
        ),
        target(
            "lists.item",
            "lists",
            "Element de listă",
            "Textul fiecărui element din listele de conținut.",
            "article li, .content li",
            Some("general.body"),
            "list-item",
            "Element de listă",
            vec![
                property("line-height", "Înălțime rând", Text, false),
                property("color", "Culoare", Color, false),
            ],
        ),
        target(
            "lists.nested",
            "lists",
            "Listă imbricată",
            "Spațierea nivelurilor secundare.",
            "article ul ul, article ol ol, .content ul ul, .content ol ol",
            Some("lists.content"),
            "nested-list",
            "Nivel principal|Nivel secundar",
            vec![
                property("margin-top", "Distanță sus", Text, false),
                property("padding-left", "Indentare", Text, false),
            ],
        ),
        target(
            "code.family",
            "quotes-code",
            "Font pentru cod",
            "Familia comună pentru fragmentele tehnice.",
            "code, kbd, samp, pre",
            None,
            "code",
            "const mesaj = \"Salut\";",
            vec![property("font-family", "Familie font", Text, false)],
        ),
        target(
            "code.inline",
            "quotes-code",
            "Cod în text",
            "Fragmente de cod afișate în interiorul unui paragraf.",
            "code",
            Some("code.family"),
            "inline-code",
            "npm run build",
            vec![
                property("font-size", "Mărime", Text, false),
                property("background-color", "Fundal", Color, false),
                property("color", "Culoare", Color, false),
                property("padding", "Spațiere", Text, false),
                property("border-radius", "Rotunjire", Text, false),
                property("border", "Contur", Text, false),
            ],
        ),
        target(
            "code.block",
            "quotes-code",
            "Bloc de cod",
            "Cod afișat pe mai multe rânduri.",
            "pre",
            Some("code.family"),
            "code-block",
            "fn main() {\n    println!(\"Salut\");\n}",
            vec![
                property("font-size", "Mărime", Text, false),
                property("background-color", "Fundal", Color, false),
                property("color", "Culoare", Color, false),
                property("padding", "Spațiere", Text, false),
                property("border-radius", "Rotunjire", Text, false),
                property("line-height", "Înălțime rând", Text, false),
            ],
        ),
        target(
            "code.keyboard",
            "quotes-code",
            "Tastă",
            "Reprezentarea unei taste sau combinații de taste.",
            "kbd",
            Some("code.family"),
            "kbd",
            "Ctrl K",
            vec![
                property("font-size", "Mărime", Text, false),
                property("background-color", "Fundal", Color, false),
                property("color", "Culoare", Color, false),
                property("padding", "Spațiere", Text, false),
                property("border-radius", "Rotunjire", Text, false),
                property("border", "Contur", Text, false),
                property("box-shadow", "Umbră", Text, false),
            ],
        ),
        target(
            "quote.block",
            "quotes-code",
            "Citat",
            "Blocul semantic de citat.",
            "blockquote",
            Some("general.body"),
            "blockquote",
            "Designul bun face lucrurile complexe să pară firești.",
            vec![
                property("border-left", "Accent lateral", Text, false),
                property("padding-left", "Spațiere laterală", Text, false),
                property("color", "Culoare", Color, false),
                choice("font-style", "Stil", false, &font_style),
            ],
        ),
        target(
            "quote.paragraph",
            "quotes-code",
            "Textul citatului",
            "Tipografia paragrafului din citat.",
            "blockquote p",
            Some("quote.block"),
            "quote-text",
            "Un citat cu greutate tipografică.",
            vec![property("font-size", "Mărime", Text, false)],
        ),
        target(
            "quote.cite",
            "quotes-code",
            "Autorul citatului",
            "Sursa sau autorul citatului.",
            "blockquote cite",
            Some("quote.block"),
            "cite",
            "— Autorul citatului",
            vec![
                property("margin-top", "Distanță sus", Text, false),
                property("font-size", "Mărime", Text, false),
                choice("font-style", "Stil", false, &font_style),
                property("color", "Culoare", Color, false),
            ],
        ),
        target(
            "table.base",
            "tables",
            "Tabel",
            "Containerul și tipografia tabelului.",
            "table",
            Some("general.body"),
            "table",
            "Produs|Cantitate|Preț",
            vec![
                property("width", "Lățime", Text, false),
                property("font-size", "Mărime text", Text, false),
                property("border-radius", "Rotunjire", Text, false),
                property("border", "Contur", Text, false),
            ],
        ),
        target(
            "table.head",
            "tables",
            "Antet tabel",
            "Fundalul rândului de antet.",
            "thead",
            Some("table.base"),
            "table-head",
            "Antet tabel",
            vec![property("background-color", "Fundal", Color, false)],
        ),
        target(
            "table.heading-cell",
            "tables",
            "Celulă de antet",
            "Tipografia și spațierea antetului.",
            "th",
            Some("table.base"),
            "table-cell",
            "Titlu coloană",
            vec![
                property("font-weight", "Greutate", Text, false),
                choice("text-align", "Aliniere", false, &text_align),
                property("padding", "Spațiere", Text, false),
                property("color", "Culoare", Color, false),
                property("border-bottom", "Separator", Text, false),
            ],
        ),
        target(
            "table.cell",
            "tables",
            "Celulă",
            "Aspectul celulelor de date.",
            "td",
            Some("table.base"),
            "table-cell",
            "Valoare",
            vec![
                property("padding", "Spațiere", Text, false),
                property("color", "Culoare", Color, false),
                property("border-bottom", "Separator", Text, false),
            ],
        ),
        target(
            "table.row-hover",
            "tables",
            "Rând la trecerea cursorului",
            "Fundalul unui rând activ.",
            "tbody tr:hover",
            Some("table.base"),
            "table-row",
            "Rând activ",
            vec![property("background-color", "Fundal", Color, false)],
        ),
        target(
            "form.label",
            "forms",
            "Etichetă câmp",
            "Textul care explică un câmp de formular.",
            "label",
            Some("general.body"),
            "label",
            "Adresă de e-mail",
            vec![
                property("font-size", "Mărime", Text, false),
                property("font-weight", "Greutate", Text, false),
                property("color", "Culoare", Color, false),
                property("margin-bottom", "Distanță jos", Text, false),
            ],
        ),
        target(
            "form.control",
            "forms",
            "Câmp de formular",
            "Aspectul comun pentru input, textarea și select.",
            "input, textarea, select",
            Some("general.body"),
            "input",
            "Conținutul câmpului",
            vec![
                property("font-family", "Familie font", Text, false),
                property("font-size", "Mărime", Text, false),
                property("color", "Culoare", Color, false),
                property("background-color", "Fundal", Color, false),
                property("border", "Contur", Text, false),
                property("border-radius", "Rotunjire", Text, false),
                property("padding", "Spațiere", Text, false),
            ],
        ),
        target(
            "form.control-hover",
            "forms",
            "Câmp la trecerea cursorului",
            "Conturul câmpului în starea hover.",
            "input:hover, textarea:hover, select:hover",
            Some("form.control"),
            "input-hover",
            "Câmp activ",
            vec![property("border-color", "Culoare contur", Color, false)],
        ),
        target(
            "form.control-focus",
            "forms",
            "Câmp focalizat",
            "Accentul vizual al câmpului focalizat; conturul accesibil rămâne protejat.",
            "input:focus, textarea:focus, select:focus",
            Some("form.control"),
            "input-focus",
            "Câmp focalizat",
            vec![
                property("border-color", "Culoare contur", Color, false),
                property("box-shadow", "Inel focalizare", Text, false),
            ],
        ),
        target(
            "form.placeholder",
            "forms",
            "Placeholder",
            "Textul ajutător afișat într-un câmp gol.",
            "input::placeholder, textarea::placeholder",
            Some("form.control"),
            "placeholder",
            "Exemplu de valoare",
            vec![property("color", "Culoare", Color, false)],
        ),
        target(
            "form.disabled",
            "forms",
            "Câmp dezactivat",
            "Opacitatea controalelor indisponibile; cursorul rămâne protejat.",
            "button:disabled, input:disabled, select:disabled, textarea:disabled",
            None,
            "disabled",
            "Control indisponibil",
            vec![property("opacity", "Opacitate", Text, false)],
        ),
        target(
            "auxiliary.separator",
            "auxiliary",
            "Separator",
            "Linia orizontală dintre secțiuni.",
            "hr",
            None,
            "separator",
            "Separator",
            vec![
                property("border-top", "Linie", Text, false),
                property("margin-block", "Spațiere verticală", Text, false),
            ],
        ),
        target(
            "auxiliary.details",
            "auxiliary",
            "Detalii",
            "Container expandabil pentru informații secundare.",
            "details",
            None,
            "details",
            "Mai multe informații",
            vec![
                property("border", "Contur", Text, false),
                property("border-radius", "Rotunjire", Text, false),
                property("padding", "Spațiere", Text, false),
            ],
        ),
        target(
            "auxiliary.summary",
            "auxiliary",
            "Rezumat detalii",
            "Titlul interactiv al containerului Detalii.",
            "summary",
            Some("auxiliary.details"),
            "summary",
            "Mai multe informații",
            vec![property("font-weight", "Greutate", Text, false)],
        ),
        target(
            "auxiliary.details-open",
            "auxiliary",
            "Detalii deschise",
            "Spațiul dintre rezumat și conținut când panoul este deschis.",
            "details[open] summary",
            Some("auxiliary.summary"),
            "details-open",
            "Conținutul este vizibil",
            vec![property("margin-bottom", "Distanță jos", Text, false)],
        ),
        target(
            "auxiliary.selection",
            "auxiliary",
            "Selecție text",
            "Culorile textului selectat de utilizator.",
            "::selection",
            None,
            "selection",
            "Text selectat",
            vec![
                property("background-color", "Fundal", Color, false),
                property("color", "Culoare", Color, false),
            ],
        ),
    ]
}

pub fn resolve_theme_style_source(
    store: &FileBufferStore,
) -> Result<(String, String, String), String> {
    let local = "sass/css-framework/_baza.scss";
    if let Some(source) = store.text_for(local) {
        return Ok((local.to_string(), source, "local".to_string()));
    }

    let active_theme = ["zola.toml", "config.toml"]
        .into_iter()
        .find_map(|path| store.text_for(path))
        .and_then(|source| active_theme_from_source(&source));
    if let Some(theme) = active_theme {
        let theme_path = format!("themes/{theme}/sass/css-framework/_baza.scss");
        if let Some(source) = store.text_for(&theme_path) {
            return Ok((theme_path, source, format!("theme:{theme}")));
        }
    }

    let mut candidates = store
        .files
        .keys()
        .filter(|path| path.ends_with("/sass/css-framework/_baza.scss"))
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort();
    match candidates.as_slice() {
        [path] => Ok((
            path.clone(),
            store.text_for(path).ok_or_else(|| {
                format!("ProjectWorkspace nu poate proiecta sursa de stil {path}.")
            })?,
            "discovered".to_string(),
        )),
        [] => Err(
            "[theme_style_source_missing] Proiectul nu conține sursa semantică sass/css-framework/_baza.scss."
                .to_string(),
        ),
        _ => Err(format!(
            "[theme_style_source_ambiguous] Proiectul conține mai multe surse _baza.scss: {}.",
            candidates.join(", ")
        )),
    }
}

pub fn collect_theme_style_variables(store: &FileBufferStore) -> BTreeMap<String, String> {
    let mut variables = BTreeMap::new();
    for (path, entry) in &store.files {
        if !path.ends_with(".scss") {
            continue;
        }
        let mut parsed = Vec::new();
        parse_variables_from_source(entry.current_text(), path, &mut parsed);
        for variable in parsed {
            variables.entry(variable.name).or_insert(variable.value);
        }
    }
    variables
}

pub fn build_theme_style_catalog(
    project_root: &str,
    runtime_session_id: &str,
    workspace_revision: u64,
    source_path: &str,
    source_origin: &str,
    source: &str,
) -> ThemeStyleCatalogSnapshot {
    let registry = style_registry();
    let mut snapshots_by_id = BTreeMap::<String, ThemeStyleTargetSnapshot>::new();
    let mut targets = Vec::with_capacity(registry.len());
    let mut warnings = Vec::new();

    for definition in &registry {
        let direct = get_exact_rule_properties(source, definition.selector).map(|properties| {
            properties
                .into_iter()
                .map(|property| (property.property, property.value))
                .collect::<BTreeMap<_, _>>()
        });
        let editable = direct.is_some();
        let diagnostic = if editable {
            None
        } else {
            let message = format!(
                "Regula exactă `{}` lipsește din {}. Editorul refuză să-i ghicească poziția.",
                definition.selector, source_path
            );
            warnings.push(message.clone());
            Some(message)
        };
        let direct = direct.unwrap_or_default();
        let parent = definition.parent_id.and_then(|id| snapshots_by_id.get(id));
        let properties = definition
            .properties
            .iter()
            .map(|property| {
                let value = direct.get(property.id).cloned();
                let inherited = if value.is_none() {
                    parent.and_then(|parent| {
                        parent
                            .properties
                            .iter()
                            .find(|candidate| candidate.id == property.id)
                            .and_then(|candidate| candidate.effective_value.clone())
                    })
                } else {
                    None
                };
                let inherited_from = inherited
                    .as_ref()
                    .and(definition.parent_id)
                    .map(str::to_string);
                ThemeStylePropertySnapshot {
                    id: property.id.to_string(),
                    label: property.label.to_string(),
                    control: property.control,
                    options: property
                        .options
                        .iter()
                        .map(|(value, label)| ThemeStyleControlOption {
                            value: (*value).to_string(),
                            label: (*label).to_string(),
                        })
                        .collect(),
                    effective_value: value.clone().or(inherited),
                    token_name: value.as_deref().and_then(exact_scss_token),
                    value,
                    inherited_from,
                    can_clear: property.can_clear,
                }
            })
            .collect::<Vec<_>>();
        let snapshot = ThemeStyleTargetSnapshot {
            id: definition.id.to_string(),
            category_id: definition.category_id.to_string(),
            label: definition.label.to_string(),
            description: definition.description.to_string(),
            selector: definition.selector.to_string(),
            parent_id: definition.parent_id.map(str::to_string),
            preview_kind: definition.preview_kind.to_string(),
            sample_text: definition.sample_text.to_string(),
            source_path: source_path.to_string(),
            editable,
            diagnostic,
            has_overrides: definition.parent_id.is_some()
                && properties.iter().any(|property| property.value.is_some()),
            properties,
        };
        snapshots_by_id.insert(snapshot.id.clone(), snapshot.clone());
        targets.push(snapshot);
    }

    let categories = style_categories()
        .into_iter()
        .map(|(id, label)| ThemeStyleCategorySnapshot {
            id: id.to_string(),
            label: label.to_string(),
            target_count: targets
                .iter()
                .filter(|target| target.category_id == id)
                .count(),
        })
        .collect();

    ThemeStyleCatalogSnapshot {
        schema_version: THEME_STYLE_CATALOG_SCHEMA_VERSION,
        project_root: project_root.to_string(),
        runtime_session_id: runtime_session_id.to_string(),
        workspace_revision,
        source_path: source_path.to_string(),
        source_origin: source_origin.to_string(),
        categories,
        targets,
        warnings,
    }
}

pub fn plan_theme_style_update(
    source_path: &str,
    source: &str,
    target_id: &str,
    inputs: &[ThemeStylePropertyInput],
) -> Result<(String, ThemeStyleTargetSnapshot), String> {
    let definition = require_target_definition(target_id)?;
    if get_exact_rule_properties(source, definition.selector).is_none() {
        return Err(format!(
            "[theme_style_rule_missing] Regula exactă `{}` lipsește din {}. Modificarea a fost refuzată.",
            definition.selector, source_path
        ));
    }
    let updates = validate_theme_style_inputs(&definition, inputs)?;
    let updated =
        update_exact_css_rule(source, definition.selector, &updates).ok_or_else(|| {
            format!(
                "[theme_style_rule_missing] Regula exactă `{}` nu mai există în {}.",
                definition.selector, source_path
            )
        })?;
    let catalog = build_theme_style_catalog("", "", 0, source_path, "draft", &updated);
    let target = catalog
        .targets
        .into_iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| format!("Ținta semantică {target_id} nu mai există în registru."))?;
    Ok((updated, target))
}

pub fn build_theme_style_preview(
    source_path: &str,
    source: &str,
    target_id: &str,
    inputs: &[ThemeStylePropertyInput],
    variables: &BTreeMap<String, String>,
) -> Result<ThemeStyleDraftPreview, String> {
    let (_, target) = plan_theme_style_update(source_path, source, target_id, inputs)?;
    let mut css = format!("{} {{\n", target.selector);
    let mut properties = Vec::new();
    for property in &target.properties {
        let Some(value) = property.effective_value.as_deref() else {
            continue;
        };
        let resolved = resolve_scss_tokens(value, variables)?;
        css.push_str(&format!("  {}: {};\n", property.id, resolved));
        properties.push(ThemeStylePreviewProperty {
            id: property.id.clone(),
            value: resolved,
            inherited: property.value.is_none(),
        });
    }
    css.push_str("}\n");
    Ok(ThemeStyleDraftPreview {
        schema_version: THEME_STYLE_CATALOG_SCHEMA_VERSION,
        target_id: target.id,
        selector: target.selector,
        source_path: source_path.to_string(),
        css,
        properties,
    })
}

fn require_target_definition(target_id: &str) -> Result<TargetDefinition, String> {
    style_registry()
        .into_iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| {
            format!(
                "[theme_style_target_unknown] Ținta semantică `{target_id}` nu există în registrul Rust."
            )
        })
}

fn validate_theme_style_inputs(
    definition: &TargetDefinition,
    inputs: &[ThemeStylePropertyInput],
) -> Result<HashMap<String, String>, String> {
    let mut seen = HashSet::new();
    let mut updates = HashMap::new();
    for input in inputs {
        if !seen.insert(input.id.as_str()) {
            return Err(format!(
                "[theme_style_property_duplicate] Proprietatea `{}` apare de două ori.",
                input.id
            ));
        }
        let property = definition
            .properties
            .iter()
            .find(|property| property.id == input.id)
            .ok_or_else(|| {
                format!(
                    "[theme_style_property_protected] Proprietatea `{}` nu este editabilă pentru `{}`.",
                    input.id, definition.id
                )
            })?;
        let value = input.value.trim();
        if value.is_empty() && !property.can_clear {
            return Err(format!(
                "[theme_style_value_required] {} nu poate reveni la moștenire.",
                property.label
            ));
        }
        if property.control == ThemeStyleControlKind::Choice
            && !value.is_empty()
            && !property.options.iter().any(|(option, _)| *option == value)
        {
            return Err(format!(
                "[theme_style_choice_invalid] Valoarea `{value}` nu este permisă pentru {}.",
                property.label
            ));
        }
        validate_panel_rule_input(
            definition.selector,
            &HashMap::from([(input.id.clone(), value.to_string())]),
            "desktop",
        )?;
        updates.insert(input.id.clone(), value.to_string());
    }
    Ok(updates)
}

fn exact_scss_token(value: &str) -> Option<String> {
    let value = value.trim();
    let name = value.strip_prefix('$')?;
    if !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        Some(name.to_string())
    } else {
        None
    }
}

fn resolve_scss_tokens(
    value: &str,
    variables: &BTreeMap<String, String>,
) -> Result<String, String> {
    let mut current = value.to_string();
    for _ in 0..16 {
        let mut next = String::with_capacity(current.len());
        let mut chars = current.char_indices().peekable();
        let mut changed = false;
        while let Some((_index, character)) = chars.next() {
            if character != '$' {
                next.push(character);
                continue;
            }
            let Some((start, first)) = chars.peek().copied() else {
                next.push('$');
                continue;
            };
            if !(first.is_ascii_alphanumeric() || matches!(first, '-' | '_')) {
                next.push('$');
                continue;
            }
            let mut end = start;
            while let Some((token_index, token_character)) = chars.peek().copied() {
                if !(token_character.is_ascii_alphanumeric()
                    || matches!(token_character, '-' | '_'))
                {
                    break;
                }
                chars.next();
                end = token_index + token_character.len_utf8();
            }
            let name = &current[start..end];
            let replacement = variables.get(name).ok_or_else(|| {
                format!(
                    "[theme_style_token_missing] Tokenul SCSS `${name}` nu poate fi rezolvat pentru previzualizare."
                )
            })?;
            next.push_str(replacement);
            changed = true;
        }
        if !changed {
            return Ok(current);
        }
        current = next;
    }
    Err(
        "[theme_style_token_cycle] Tokenii SCSS formează un ciclu sau depășesc limita de rezolvare."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"
body { min-height: 100dvh; font-family: $font-primary; color: $text-body; background-color: $bg-body; }
h1, h2, h3, h4, h5, h6 { font-family: $font-display; color: $text-heading; }
h1 { font-size: 3rem; }
input:focus, textarea:focus, select:focus { border-color: $color-primary; box-shadow: 0 0 0 3px blue; outline: none; }
"#;

    #[test]
    fn registry_covers_every_public_category_and_keeps_protected_properties_out() {
        let registry = style_registry();
        for (category, _) in style_categories() {
            assert!(
                registry.iter().any(|target| target.category_id == category),
                "missing category {category}"
            );
        }
        let body = registry
            .iter()
            .find(|target| target.id == "general.body")
            .unwrap();
        assert!(!body
            .properties
            .iter()
            .any(|property| property.id == "min-height"));
        let focus = registry
            .iter()
            .find(|target| target.id == "form.control-focus")
            .unwrap();
        assert!(!focus
            .properties
            .iter()
            .any(|property| property.id == "outline"));
    }

    #[test]
    fn heading_override_can_return_to_group_inheritance_without_losing_anchor() {
        let (with_override, heading) = plan_theme_style_update(
            "_baza.scss",
            BASE,
            "typography.h1",
            &[
                ThemeStylePropertyInput {
                    id: "font-size".to_string(),
                    value: "4rem".to_string(),
                },
                ThemeStylePropertyInput {
                    id: "color".to_string(),
                    value: "#123456".to_string(),
                },
            ],
        )
        .unwrap();
        assert!(with_override.contains("font-size: 4rem"));
        assert_eq!(
            heading
                .properties
                .iter()
                .find(|property| property.id == "font-size")
                .and_then(|property| property.value.as_deref()),
            Some("4rem")
        );

        let (inherited, heading) = plan_theme_style_update(
            "_baza.scss",
            &with_override,
            "typography.h1",
            &[ThemeStylePropertyInput {
                id: "color".to_string(),
                value: String::new(),
            }],
        )
        .unwrap();
        assert!(!inherited.contains("#123456"));
        let color = heading
            .properties
            .iter()
            .find(|property| property.id == "color")
            .unwrap();
        assert_eq!(color.value, None);
        assert_eq!(color.effective_value.as_deref(), Some("$text-heading"));
        assert_eq!(color.inherited_from.as_deref(), Some("typography.headings"));
    }

    #[test]
    fn protected_property_and_unknown_target_are_rejected() {
        let protected = plan_theme_style_update(
            "_baza.scss",
            BASE,
            "general.body",
            &[ThemeStylePropertyInput {
                id: "min-height".to_string(),
                value: "0".to_string(),
            }],
        )
        .unwrap_err();
        assert!(protected.contains("theme_style_property_protected"));

        let unknown =
            plan_theme_style_update("_baza.scss", BASE, "technical.reset", &[]).unwrap_err();
        assert!(unknown.contains("theme_style_target_unknown"));
    }

    #[test]
    fn preview_resolves_nested_scss_tokens_without_important() {
        let variables = BTreeMap::from([
            ("text-heading".to_string(), "$color-primary".to_string()),
            ("color-primary".to_string(), "#155eef".to_string()),
            (
                "font-display".to_string(),
                "Display, sans-serif".to_string(),
            ),
        ]);
        let preview = build_theme_style_preview(
            "_baza.scss",
            BASE,
            "typography.headings",
            &[
                ThemeStylePropertyInput {
                    id: "font-family".to_string(),
                    value: "$font-display".to_string(),
                },
                ThemeStylePropertyInput {
                    id: "color".to_string(),
                    value: "$text-heading".to_string(),
                },
            ],
            &variables,
        )
        .unwrap();
        assert!(preview.css.contains("#155eef"));
        assert!(!preview.css.contains("!important"));
    }

    #[test]
    fn bundled_framework_has_every_exact_semantic_anchor() {
        let source = include_str!(
            "../../../resources/theme-packs/pana-studio/theme/sass/css-framework/_baza.scss"
        );
        let catalog = build_theme_style_catalog(
            "/project",
            "runtime",
            7,
            "themes/pana-studio/sass/css-framework/_baza.scss",
            "theme:pana-studio",
            source,
        );
        assert!(catalog.warnings.is_empty(), "{:?}", catalog.warnings);
        assert!(catalog.targets.iter().all(|target| target.editable));
    }
}

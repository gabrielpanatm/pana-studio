import type { EditableStyles } from "$lib/types";

export const editableStylePropertyMap: Record<keyof EditableStyles, string> = {
  color: "color",
  backgroundColor: "background-color",
  fontSize: "font-size",
  lineHeight: "line-height",
  textAlign: "text-align",
  margin: "margin",
  padding: "padding",
  borderRadius: "border-radius",
  display: "display",
  flexDirection: "flex-direction",
  gap: "gap",
  justifyContent: "justify-content",
  alignItems: "align-items",
};

export function buildDirtyStyleProperties(
  dirty: ReadonlySet<keyof EditableStyles>,
  styles: EditableStyles,
): Record<string, string> {
  const properties: Record<string, string> = {};
  for (const key of dirty) {
    const cssProperty = editableStylePropertyMap[key];
    if (cssProperty && styles[key]) {
      properties[cssProperty] = styles[key];
    }
  }
  return properties;
}

export function serializeOverrides(
  rules: Record<string, EditableStyles>,
  variables: Record<string, string>,
) {
  const variableEntries = Object.entries(variables).filter(([, value]) => value.trim().length > 0);
  const blocks = Object.entries(rules).map(([selector, styles]) =>
    [
      `${selector} {`,
      `  color: ${styles.color};`,
      `  background-color: ${styles.backgroundColor};`,
      `  font-size: ${styles.fontSize};`,
      `  line-height: ${styles.lineHeight};`,
      `  text-align: ${styles.textAlign};`,
      `  margin: ${styles.margin};`,
      `  padding: ${styles.padding};`,
      `  border-radius: ${styles.borderRadius};`,
      `  display: ${styles.display};`,
      `  flex-direction: ${styles.flexDirection};`,
      `  gap: ${styles.gap};`,
      `  justify-content: ${styles.justifyContent};`,
      `  align-items: ${styles.alignItems};`,
      `}`,
    ].join("\n"),
  );

  const variableBlock =
    variableEntries.length > 0
      ? [
          ":root {",
          ...variableEntries.map(([name, value]) => `  ${name}: ${value};`),
          "}",
        ].join("\n")
      : null;

  return [
    ...(variableBlock ? [variableBlock, ""] : []),
    ...blocks.flatMap((block) => [block, ""]),
  ].join("\n");
}

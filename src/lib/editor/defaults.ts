import type { EditableStyles } from "$lib/types";

export function createDefaultEditableStyles(): EditableStyles {
  return {
    color: "#17211d",
    backgroundColor: "#ffffff",
    fontSize: "16px",
    lineHeight: "normal",
    textAlign: "left",
    margin: "0px",
    padding: "0px",
    borderRadius: "0px",
    display: "block",
    flexDirection: "row",
    gap: "0px",
    justifyContent: "normal",
    alignItems: "normal",
  };
}

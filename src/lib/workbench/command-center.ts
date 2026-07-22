import { invoke } from "@tauri-apps/api/core";
import {
  COMMAND_CENTER_SCHEMA_VERSION,
  type CommandCenterScope,
  type CommandCenterSearchResponse,
} from "$lib/types";

export async function searchCommandCenter(input: {
  query: string;
  scope?: CommandCenterScope;
  limit?: number;
  projectRoot?: string | null;
  runtimeSessionId?: string | null;
}): Promise<CommandCenterSearchResponse> {
  const request = {
    query: input.query,
    scope: input.scope ?? "all",
    limit: input.limit ?? 40,
    expectedProjectRoot: input.projectRoot ?? null,
    expectedSessionId: input.runtimeSessionId ?? null,
  };
  const response = await invoke<CommandCenterSearchResponse>("search_command_center", {
    request,
  });
  requireCommandCenterResponse(response, request.expectedProjectRoot, request.expectedSessionId);
  return response;
}

function requireCommandCenterResponse(
  response: CommandCenterSearchResponse,
  projectRoot: string | null,
  runtimeSessionId: string | null,
) {
  if (response.schemaVersion !== COMMAND_CENTER_SCHEMA_VERSION) {
    throw new Error(
      "Command Center schema incompatibilă: " + response.schemaVersion + ".",
    );
  }
  if (
    response.projectRoot !== projectRoot
    || response.runtimeSessionId !== runtimeSessionId
  ) {
    throw new Error("Command Center a returnat rezultate pentru altă ProjectSession.");
  }
  if (!Array.isArray(response.results)) {
    throw new Error("Command Center nu a returnat o listă validă de rezultate.");
  }
}

export function commandCenterQuery(input: string): {
  query: string;
  scope: CommandCenterScope;
  scopeLabel: string;
} {
  const trimmedStart = input.trimStart();
  if (trimmedStart.startsWith(">")) {
    return {
      query: trimmedStart.slice(1).trimStart(),
      scope: "commands",
      scopeLabel: "Comenzi",
    };
  }
  if (trimmedStart.startsWith("@")) {
    return {
      query: trimmedStart.slice(1).trimStart(),
      scope: "symbols",
      scopeLabel: "Simboluri",
    };
  }
  if (trimmedStart.startsWith("#")) {
    return {
      query: trimmedStart.slice(1).trimStart(),
      scope: "files",
      scopeLabel: "Fișiere",
    };
  }
  return { query: input, scope: "all", scopeLabel: "Tot" };
}

import { invoke } from "@tauri-apps/api/core";
import {
  COMMAND_CENTER_SCHEMA_VERSION,
  type CommandCenterScope,
  type CommandCenterSearchResponse,
} from "$lib/types";
import { t } from "$lib/i18n/runtime.svelte";

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
      t("command-center-schema-mismatch", {
        actual: response.schemaVersion,
        expected: COMMAND_CENTER_SCHEMA_VERSION,
      }),
    );
  }
  if (
    response.projectRoot !== projectRoot
    || response.runtimeSessionId !== runtimeSessionId
  ) {
    throw new Error(t("command-center-session-mismatch"));
  }
  if (!Array.isArray(response.results)) {
    throw new Error(t("command-center-results-invalid"));
  }
}

export function commandCenterQuery(input: string): {
  query: string;
  scope: CommandCenterScope;
} {
  const trimmedStart = input.trimStart();
  if (trimmedStart.startsWith(">")) {
    return {
      query: trimmedStart.slice(1).trimStart(),
      scope: "commands",
    };
  }
  if (trimmedStart.startsWith("@")) {
    return {
      query: trimmedStart.slice(1).trimStart(),
      scope: "symbols",
    };
  }
  if (trimmedStart.startsWith("#")) {
    return {
      query: trimmedStart.slice(1).trimStart(),
      scope: "files",
    };
  }
  return { query: input, scope: "all" };
}

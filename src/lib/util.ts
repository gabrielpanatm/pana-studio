type DiagnosticArguments = Record<string, string | number>;
type DiagnosticFormatter = (
  code: string,
  arguments_: DiagnosticArguments,
) => string | null;

let diagnosticFormatter: DiagnosticFormatter | null = null;

export function registerDiagnosticFormatter(formatter: DiagnosticFormatter) {
  diagnosticFormatter = formatter;
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") {
    const parsed = parseJsonRecord(error);
    if (!parsed) return error;
    error = parsed;
  }

  const record = asRecord(error);
  if (record) {
    const code = stringField(record, "code");
    if (code && diagnosticFormatter) {
      const formatted = diagnosticFormatter(
        code,
        primitiveArguments(asRecord(record.arguments)),
      );
      if (formatted) return formatted;
    }
    const detail = asRecord(record.detail);
    const diagnostic = stringField(detail, "diagnostic") ?? stringField(record, "diagnostic");
    const kind = stringField(record, "kind");

    if (kind === "recovery_required" && detail && diagnostic) {
      const operationId =
        stringField(detail, "commandId") ??
        stringField(detail, "transactionId") ??
        stringField(asRecord(detail.receipt), "id");
      const phase = stringField(detail, "phase");
      const context = [operationId, phase].filter(Boolean).join(", ");
      const contextLabel = context ? ` [${context}]` : "";
      const localized = diagnosticFormatter?.("recovery-required-error", {
        context: contextLabel,
        diagnostic,
      });
      return localized
        ?? `RECOVERY_REQUIRED${contextLabel}: ${diagnostic} Do not retry automatically.`;
    }

    if (diagnostic) return diagnostic;
    const message = stringField(record, "message");
    if (message) return message;

    try {
      return JSON.stringify(error);
    } catch {
      // Fall through to the final stable diagnostic.
    }
  }

  return String(error);
}

export function isRecoveryRequiredError(error: unknown): boolean {
  return asRecord(error)?.kind === "recovery_required";
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null
    ? value as Record<string, unknown>
    : null;
}

function stringField(record: Record<string, unknown> | null, key: string): string | null {
  const value = record?.[key];
  return typeof value === "string" && value.trim() ? value : null;
}

function primitiveArguments(
  record: Record<string, unknown> | null,
): DiagnosticArguments {
  if (!record) return {};
  return Object.fromEntries(
    Object.entries(record).filter(
      (entry): entry is [string, string | number] =>
        typeof entry[1] === "string" || typeof entry[1] === "number",
    ),
  );
}

function parseJsonRecord(value: string): Record<string, unknown> | null {
  if (!value.trimStart().startsWith("{")) return null;
  try {
    return asRecord(JSON.parse(value));
  } catch {
    return null;
  }
}

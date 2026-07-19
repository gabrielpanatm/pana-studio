export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;

  const record = asRecord(error);
  if (record) {
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
      return `RECOVERY_REQUIRED${context ? ` [${context}]` : ""}: ${diagnostic} Nu repeta operația automat.`;
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

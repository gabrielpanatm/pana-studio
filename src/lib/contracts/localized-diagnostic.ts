export type LocalizedDiagnostic = {
  schemaVersion: number;
  code: string;
  arguments?: Record<string, string | number>;
};

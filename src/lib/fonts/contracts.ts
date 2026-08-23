import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type {
  ProjectWorkspaceMutationReceipt,
  ProjectWorkspaceSnapshot,
} from "$lib/project/workspace-contract";

type FontOrigin = "bundled" | "local" | "theme" | "external";

type FontDeliveryKind = "local" | "system" | "external" | "missing";

type FontRoleDeliveryKind = FontDeliveryKind;

type FontOwnership = "managed" | "detected";

type FontRoot = {
  relativePath: string;
  origin: FontOrigin;
  themeName: string | null;
  exists: boolean;
};

type LocalFontFile = {
  file: string;
  fileName: string;
  sizeBytes: number;
  extension: string;
  format: string;
  textOptimized: boolean;
  contentHash: string;
  internalFamily: string | null;
  subfamily: string | null;
  weight: number | null;
  weightRange: FontWeightRange | null;
  style: string | null;
  axes: FontVariationAxis[];
  license: FontLicenseMetadata;
  unicodeRange: string | null;
  romanianGlyphs: string[];
  declaredWeight: number | null;
  declaredWeightRange: FontWeightRange | null;
  declaredStyle: string | null;
  preload: FontPreloadRegistration;
};

type FontPreloadRegistration = {
  preloaded: boolean;
  managed: boolean;
  templates: string[];
};

type FontVariationAxis = {
  tag: string;
  min: number;
  default: number;
  max: number;
};

export type InstalledFontVariationAxis = FontVariationAxis & {
  family: string;
};

type FontLicenseMetadata = {
  description: string | null;
  url: string | null;
};

type FontWeightRange = {
  start: number;
  end: number;
};

type FontCssRegistration = {
  registered: boolean;
  managed: boolean;
  stylesheets: string[];
  displayModes: string[];
};

type FontFaceIssueSeverity = "info" | "warning" | "error";

type FontFaceIssue = {
  code: string;
  severity: FontFaceIssueSeverity;
  message: string;
  file: string | null;
  stylesheet: string | null;
};

type FontFaceSource = {
  stylesheet: string;
  url: string;
  resolvedFile: string | null;
  delivery: FontDeliveryKind;
  ownership: FontOwnership;
  external: boolean;
  dynamic: boolean;
  weight: number | null;
  weightRange: FontWeightRange | null;
  style: string;
  display: string | null;
  unicodeRange: string | null;
  managed: boolean;
};

type FontFaceFamily = {
  id: string;
  family: string;
  directories: string[];
  origin: FontOrigin;
  themeName: string | null;
  delivery: FontDeliveryKind;
  ownership: FontOwnership;
  romanianSupported: boolean | null;
  files: LocalFontFile[];
  faces: FontFaceSource[];
  issues: FontFaceIssue[];
  license: FontLicenseMetadata;
  registration: FontCssRegistration;
};

type FontFaceGraph = {
  schemaVersion: number;
  roots: FontRoot[];
  families: FontFaceFamily[];
};

export type FontRoleId = "text" | "titles" | "ui" | "mono";

type FontRoleAssignment = {
  id: FontRoleId;
  label: string;
  variableName: string;
  value: string | null;
  family: string | null;
  sourcePath: string | null;
  delivery: FontRoleDeliveryKind;
  installed: boolean;
  assignable: boolean;
  diagnostic: string | null;
};

export type FontManagerSnapshot = {
  schemaVersion: number;
  graph: FontFaceGraph;
  roles: FontRoleAssignment[];
  diagnostics: FontDeliveryDiagnostic[];
};

type FontDeliveryDiagnosticSeverity = "info" | "warning" | "error";

type FontDeliveryDiagnostic = {
  severity: FontDeliveryDiagnosticSeverity;
  code: string;
  messageDiagnostic: LocalizedDiagnostic;
  family: string | null;
  file: string | null;
};

export type FontRoleAssignmentReceipt = {
  role: FontRoleAssignment;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
  manager: FontManagerSnapshot;
};

export type FontDeliveryMutationReceipt = {
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
  manager: FontManagerSnapshot;
};

export type FontFamilyRemovalPlan = {
  schemaVersion: number;
  planToken: string;
  familyId: string;
  family: string;
  directories: string[];
  files: string[];
  stylesheetPaths: string[];
  licenseFiles: string[];
  retainedResources: string[];
  blockedReasons: string[];
  warnings: string[];
  changed: boolean;
};

export type FontFamilyRemovalReceipt = {
  plan: FontFamilyRemovalPlan;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
  manager: FontManagerSnapshot;
};

export type FontPreviewAsset = {
  file: string;
  format: string;
  dataUrl: string;
  contentHash: string;
};

export type BundledFontCatalogFamily = {
  id: string;
  family: string;
  category: string;
  lastModified: string;
  specimenUrl: string;
  cssUrl: string;
  styles: string[];
  weightRange: FontWeightRange;
  fileCount: number;
  sizeBytes: number;
  variable: boolean;
  romanianSupported: boolean;
  license: FontLicenseMetadata;
  licenseFile: string;
};

type BundledFontPreviewFace = {
  libraryPath: string;
  subset: string;
  unicodeRange: string;
  style: string;
  weightRange: FontWeightRange;
  format: string;
  dataUrl: string;
  contentHash: string;
  sourceUrl: string;
};

export type BundledFontPreview = {
  familyId: string;
  family: string;
  faces: BundledFontPreviewFace[];
};

export type BundledFontInstallReceipt = {
  family: FontFaceFamily;
  licenseFile: string;
  licenseSourceUrl: string;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
};

type GoogleFontDownloadResult = {
  family: FontFaceFamily;
  fontFaceCss: string;
  cssUrl: string;
  licenseFile: string;
  licenseSourceUrl: string;
  variable: boolean;
  textOptimized: boolean;
  optimizedCharacterCount: number;
};

export type GoogleFontInstallReceipt = {
  result: GoogleFontDownloadResult;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
};

type LocalFontImportFilePlan = {
  sourcePath: string;
  destinationPath: string;
  family: string;
  subfamily: string | null;
  sizeBytes: number;
  extension: string;
  format: string;
  weight: number | null;
  weightRange: FontWeightRange | null;
  style: string;
  axes: FontVariationAxis[];
};

type LocalFontImportFamilyPlan = {
  id: string;
  family: string;
  directory: string;
  fileCount: number;
  variable: boolean;
  license: FontLicenseMetadata;
};

export type LocalFontImportPlan = {
  schemaVersion: number;
  planToken: string;
  stylesheetPath: string;
  families: LocalFontImportFamilyPlan[];
  files: LocalFontImportFilePlan[];
  warnings: string[];
  conflicts: string[];
  changed: boolean;
};

export type LocalFontImportReceipt = {
  plan: LocalFontImportPlan;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
};

export type GoogleFontAxis = {
  tag: string;
  start: number;
  end: number;
};

export type GoogleFontCatalogFamily = {
  family: string;
  category: string | null;
  variants: string[];
  weights: number[];
  subsets: string[];
  axes: GoogleFontAxis[];
};

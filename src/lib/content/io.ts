import type {
  FileBufferRequestIdentity,
  WorkspaceEntryMutationReceipt,
} from "$lib/project/workspace-contract";
import type {
  PageFrontmatterField,
  PageFrontmatterMutationValue,
} from "$lib/markdown/frontmatter";
import { invokeWorkspaceEntryMutation } from "$lib/session/workspace-entry-io";

export function createProjectContentPage(options: {
  section: string;
  slug: string;
  title: string;
}, identity: FileBufferRequestIdentity): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_create_content_page", { ...options, identity }, identity);
}

export function updateProjectPageFrontmatterField(
  input: {
    relativePath: string;
    field: PageFrontmatterField;
    value: PageFrontmatterMutationValue;
  },
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_update_page_frontmatter_field",
    { input, identity },
    identity,
  );
}

export function createProjectTextFile(
  relativePath: string,
  contents: string,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_create_project_text_file",
    { relativePath, contents, identity },
    identity,
  );
}

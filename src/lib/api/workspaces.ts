import { invokeCommand } from "$lib/api/transport";
import type { WorkspaceProfile } from "$lib/types";

export async function listWorkspaces(): Promise<WorkspaceProfile[]> {
  return invokeCommand<WorkspaceProfile[]>("list_workspaces");
}

export async function createWorkspace(
  path: string,
  name?: string,
): Promise<WorkspaceProfile> {
  return invokeCommand<WorkspaceProfile>("create_workspace", { path, name });
}

export async function updateWorkspace(profile: WorkspaceProfile): Promise<void> {
  return invokeCommand<void>("update_workspace", { profile });
}

export async function deleteWorkspace(id: string): Promise<void> {
  return invokeCommand<void>("delete_workspace", { id });
}

import { invokeCommand } from "$lib/api/transport";
import type { RuntimeStatus, WorkspaceProfile } from "$lib/types";

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

export async function openWorkspaceDirectory(path: string): Promise<void> {
  return invokeCommand<void>("open_workspace_directory", { path });
}

export async function deleteWorkspace(id: string): Promise<void> {
  return invokeCommand<void>("delete_workspace", { id });
}

export async function startRuntime(id: string): Promise<RuntimeStatus> {
  return invokeCommand<RuntimeStatus>("start_runtime", { id });
}

export async function stopRuntime(id: string): Promise<RuntimeStatus> {
  return invokeCommand<RuntimeStatus>("stop_runtime", { id });
}

export async function getRuntimeStatus(id: string): Promise<RuntimeStatus> {
  return invokeCommand<RuntimeStatus>("get_runtime_status", { id });
}

export async function startActionsRuntime(id: string): Promise<RuntimeStatus> {
  return invokeCommand<RuntimeStatus>("start_actions_runtime", { id });
}

export async function stopActionsRuntime(id: string): Promise<RuntimeStatus> {
  return invokeCommand<RuntimeStatus>("stop_actions_runtime", { id });
}

export async function getActionsRuntimeStatus(id: string): Promise<RuntimeStatus> {
  return invokeCommand<RuntimeStatus>("get_actions_runtime_status", { id });
}

export async function restartRuntime(id: string): Promise<RuntimeStatus> {
  return invokeCommand<RuntimeStatus>("restart_runtime", { id });
}

export async function restartActionsRuntime(id: string): Promise<RuntimeStatus> {
  return invokeCommand<RuntimeStatus>("restart_actions_runtime", { id });
}

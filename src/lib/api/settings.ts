import { invokeCommand } from "$lib/api/transport";

export interface FrpProfileDto {
  id: string;
  name: string;
  server: string;
  serverPort: number;
  hasToken: boolean;
}

export interface FrpProfileInput {
  id: string;
  name: string;
  server: string;
  serverPort: number;
}

export async function listFrpProfiles(): Promise<FrpProfileDto[]> {
  return invokeCommand<FrpProfileDto[]>("list_frp_profiles");
}

export async function saveFrpProfile(
  profile: FrpProfileInput,
  token?: string,
): Promise<FrpProfileDto> {
  return invokeCommand<FrpProfileDto>("save_frp_profile", { profile, token });
}

export async function getLastWorkspaceId(): Promise<string> {
  return invokeCommand<string>("get_last_workspace_id");
}

export async function setLastWorkspace(id: string): Promise<void> {
  return invokeCommand<void>("set_last_workspace", { id });
}

export async function deleteFrpProfile(id: string): Promise<void> {
  return invokeCommand<void>("delete_frp_profile", { id });
}

export interface ProxyConfigDto {
  mode: string;
  url: string;
}

export async function getProxy(): Promise<ProxyConfigDto> {
  return invokeCommand<ProxyConfigDto>("get_proxy");
}

export async function setProxy(proxy: ProxyConfigDto): Promise<void> {
  return invokeCommand<void>("set_proxy", { proxy });
}

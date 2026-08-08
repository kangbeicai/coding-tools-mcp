import { invokeCommand } from "$lib/api/transport";

export interface GatewayConfig {
  bindHost: string;
  localPort: number;
  publicUrl: string;
  authType: "oauth" | "bearer" | "noauth" | string;
  toolProfile: string;
  autoSelectSingleWorkspace: boolean;
}

export interface GatewaySessionInfo {
  sessionKey: string;
  workspaceId: string;
  workspaceName: string;
  selectedAtUnixMs: number;
}

export interface GatewayStatus {
  state: string;
  localEndpoint: string;
  publicEndpoint: string;
  workspaceCount: number;
  sessionCount: number;
  sessions: GatewaySessionInfo[];
}

export function getGatewayConfig(): Promise<GatewayConfig> {
  return invokeCommand<GatewayConfig>("get_gateway_config");
}

export function setGatewayConfig(gateway: GatewayConfig): Promise<void> {
  return invokeCommand<void>("set_gateway_config", { gateway });
}

export function getGatewayStatus(): Promise<GatewayStatus> {
  return invokeCommand<GatewayStatus>("get_gateway_status");
}

export function startGateway(): Promise<GatewayStatus> {
  return invokeCommand<GatewayStatus>("start_gateway");
}

export function stopGateway(): Promise<GatewayStatus> {
  return invokeCommand<GatewayStatus>("stop_gateway");
}

export function restartGateway(): Promise<GatewayStatus> {
  return invokeCommand<GatewayStatus>("restart_gateway");
}

export async function clearGatewaySession(sessionKey: string): Promise<boolean> {
  const result = await invokeCommand<{ removed: boolean }>("clear_gateway_session", { sessionKey });
  return result.removed;
}


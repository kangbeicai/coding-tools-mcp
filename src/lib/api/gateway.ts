import { invokeCommand } from "$lib/api/transport";

export interface GatewayConfig {
  bindHost: string;
  localPort: number;
  publicUrl: string;
  authType: "oauth" | "bearer" | "noauth" | string;
  toolProfile: string;
  autoSelectSingleWorkspace: boolean;
}

export interface GatewayHealthItem {
  key: string;
  layer: string;
  label: string;
  status: "ok" | "warn" | "fail" | "skip" | string;
  detail: string;
  hint: string;
}

export interface GatewayHealthReport {
  chatgptReady: boolean;
  summary: string;
  publicBaseUrl: string;
  items: GatewayHealthItem[];
}

export function getGatewayExposure(): Promise<GatewayExposureConfig> {
  return invokeCommand<GatewayExposureConfig>("get_gateway_exposure");
}

export function setGatewayExposure(exposure: GatewayExposureConfig): Promise<void> {
  return invokeCommand<void>("set_gateway_exposure", { exposure });
}

export function getGatewayExposureStatus(): Promise<GatewayExposureStatus> {
  return invokeCommand<GatewayExposureStatus>("get_gateway_exposure_status");
}

export function startGatewayExposure(): Promise<GatewayExposureStatus> {
  return invokeCommand<GatewayExposureStatus>("start_gateway_exposure");
}

export function stopGatewayExposure(): Promise<GatewayExposureStatus> {
  return invokeCommand<GatewayExposureStatus>("stop_gateway_exposure");
}

export interface GatewayExposureConfig {
  mode: "local" | "direct" | "external" | "frp" | "cloudflare" | string;
  frpProfileId: string;
  frpServer: string;
  frpServerPort: number;
  frpSubdomain: string;
  cloudflareMode: "quick" | "named" | string;
  useProxy: boolean;
}

export interface GatewayExposureStatus {
  state: string;
  mode: string;
  managed: boolean;
  canonicalPublicUrl: string;
  effectivePublicUrl: string;
  pid: number | null;
  message: string;
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

export function runGatewayHealthChecks(): Promise<GatewayHealthReport> {
  return invokeCommand<GatewayHealthReport>("run_gateway_health_checks");
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


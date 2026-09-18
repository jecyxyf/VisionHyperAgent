import type { AgentConfig, CodexConfig, ProviderConfig } from "../components/settings/types";

export interface AgentSettingsResponse {
  revision: string;
  configPath: string;
  agent: AgentConfig;
}

export interface SaveAgentSettingsResponse {
  ok: boolean;
  revision: string;
  restartRequired: boolean;
}

interface ProviderConfigRequest {
  id: string;
  baseUrl: string;
  apiKey: string;
  wireApi: ProviderConfig["wireApi"];
  models: ProviderConfig["models"];
}

interface AgentConfigRequest {
  activeModelId: string;
  providers: ProviderConfigRequest[];
  codex: CodexConfig;
}

interface SettingsErrorBody {
  error?: { code?: string; message?: string };
}

export class AgentSettingsError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(message: string, code = "settings_request", status = 0) {
    super(message);
    this.name = "AgentSettingsError";
    this.code = code;
    this.status = status;
  }
}

export async function loadAgentSettings(): Promise<AgentSettingsResponse> {
  const response = await fetch("/api/settings/agent");
  const result = await parseResponse<AgentSettingsResponse>(response);
  if (!response.ok) throw settingsError(result, response.status);
  return normalizeResponse(result);
}

export async function saveAgentSettings(
  revision: string,
  agent: AgentConfig,
): Promise<SaveAgentSettingsResponse> {
  const response = await fetch("/api/settings/agent", {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-VHA-Settings": "1" },
    body: JSON.stringify({ revision, agent: toRequestAgent(agent) }),
  });
  const result = await parseResponse<SaveAgentSettingsResponse>(response);
  if (!response.ok || result.ok !== true) throw settingsError(result, response.status);
  return result;
}

export function toRequestAgent(agent: AgentConfig): AgentConfigRequest {
  return {
    activeModelId: agent.activeModelId.trim(),
    providers: agent.providers.map((provider) => ({
      id: provider.id.trim(),
      baseUrl: provider.baseUrl.trim(),
      apiKey: provider.apiKey.trim(),
      wireApi: provider.wireApi,
      models: provider.models.map((model) => ({
        ...model,
        modelId: model.modelId.trim(),
        modelName: model.modelName.trim(),
        supportedEfforts: [...model.supportedEfforts],
        effort: model.effort,
      })),
    })),
    codex: toRequestCodex(agent.codex),
  };
}

function toRequestCodex(codex: CodexConfig): CodexConfig {
  return {
    ...codex,
    executable: codex.executable?.trim() ? codex.executable.trim() : null,
    workspace: codex.workspace.trim(),
    home: codex.home.trim(),
  };
}

async function parseResponse<T>(response: Response): Promise<T> {
  try {
    return (await response.json()) as T;
  } catch {
    throw new AgentSettingsError("设置接口返回格式错误", "invalid_response", response.status);
  }
}

function settingsError(result: unknown, status: number): AgentSettingsError {
  const body = result as SettingsErrorBody | undefined;
  return new AgentSettingsError(
    body?.error?.message ?? "设置请求失败，请检查后端状态。",
    body?.error?.code ?? "settings_request",
    status,
  );
}

function normalizeResponse(value: AgentSettingsResponse): AgentSettingsResponse {
  return {
    revision: value.revision,
    configPath: value.configPath,
    agent: {
      ...value.agent,
      providers: value.agent.providers.map(normalizeProvider),
      codex: { ...value.agent.codex, executable: value.agent.codex.executable ?? null },
    },
  };
}

function normalizeProvider(provider: ProviderConfig): ProviderConfig {
  return { ...provider, apiKey: "", apiKeyConfigured: Boolean(provider.apiKeyConfigured) };
}

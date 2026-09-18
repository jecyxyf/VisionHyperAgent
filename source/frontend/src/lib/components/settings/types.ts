export const SUPPORTED_EFFORTS = ["low", "medium", "high", "xhigh", "max", "ultra"] as const;

export type ReasoningEffort = (typeof SUPPORTED_EFFORTS)[number];
export type ProviderWireApi = "responses" | "chat_completions";

export interface ModelConfig {
  modelId: string;
  modelName: string;
  supportedEfforts: string[];
  effort: string;
  supportsImages: boolean;
}

export interface ProviderConfig {
  id: string;
  baseUrl: string;
  apiKey: string;
  apiKeyConfigured: boolean;
  wireApi: ProviderWireApi;
  models: ModelConfig[];
}

export interface CodexConfig {
  executable: string | null;
  workspace: string;
  home: string;
  connectTimeoutMs: number;
  requestTimeoutMs: number;
  reconnectIntervalMs: number;
  maxReconnectAttempts: number;
  approvalTimeoutMs: number;
  eventCapacity: number;
  experimentalApi: boolean;
}

export interface AgentConfig {
  activeModelId: string;
  providers: ProviderConfig[];
  codex: CodexConfig;
}

export function newModel(): ModelConfig {
  return {
    modelId: "",
    modelName: "",
    supportedEfforts: [...SUPPORTED_EFFORTS],
    effort: "medium",
    supportsImages: true,
  };
}

export function newProvider(): ProviderConfig {
  return {
    id: "",
    baseUrl: "https://",
    apiKey: "",
    apiKeyConfigured: false,
    wireApi: "chat_completions",
    models: [newModel()],
  };
}

export function allModels(agent: AgentConfig): Array<{ providerId: string; model: ModelConfig }> {
  return agent.providers.flatMap((provider) => provider.models.map((model) => ({ providerId: provider.id, model })));
}

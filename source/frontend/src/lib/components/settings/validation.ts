import type { AgentConfig } from "./types";

export interface ValidationIssue {
  path: string;
  message: string;
}

export function validateAgentSettings(agent: AgentConfig): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  const providerIds = new Set<string>();
  const modelIds = new Map<string, string>();

  agent.providers.forEach((provider, providerIndex) => {
    const providerPath = `providers[${providerIndex}]`;
    const providerId = provider.id.trim();
    if (!providerId) push(issues, providerPath, "Provider ID 不能为空");
    else if (providerIds.has(providerId)) push(issues, providerPath, `Provider ID 重复：${providerId}`);
    providerIds.add(providerId);

    if (!provider.baseUrl.trim()) push(issues, providerPath, "Base URL 不能为空");
    else if (!isSafeHttpUrl(provider.baseUrl)) push(issues, providerPath, "Base URL 必须是无凭据、无 query 的 HTTP(S) 地址");
    if (!provider.apiKey.trim() && !provider.apiKeyConfigured) push(issues, providerPath, "新 Provider 必须填写 API Key");
    if (!provider.models.length) push(issues, providerPath, "Provider 至少需要一个模型");

    provider.models.forEach((model, modelIndex) => {
      const modelPath = `${providerPath}.models[${modelIndex}]`;
      const modelId = model.modelId.trim();
      if (!modelId) push(issues, modelPath, "modelId 不能为空");
      else if (modelIds.has(modelId)) push(issues, modelPath, `modelId 全局重复：${modelId}`);
      modelIds.set(modelId, providerId);
      if (!model.modelName.trim()) push(issues, modelPath, "modelName 不能为空");
      if (!model.supportedEfforts.length) push(issues, modelPath, "至少选择一个 Effort");
      if (new Set(model.supportedEfforts).size !== model.supportedEfforts.length) push(issues, modelPath, "Effort 不能重复");
      if (model.supportedEfforts.length && !model.supportedEfforts.includes(model.effort)) {
        push(issues, modelPath, "默认 Effort 必须属于 supportedEfforts");
      }
    });
  });

  if (agent.providers.length && !modelIds.has(agent.activeModelId)) {
    push(issues, "activeModelId", "默认模型必须来自当前模型列表");
  }
  validateCodex(agent, issues);
  return issues;
}

function validateCodex(agent: AgentConfig, issues: ValidationIssue[]) {
  const codex = agent.codex;
  if (!codex.workspace.trim()) push(issues, "codex.workspace", "Codex 工作目录不能为空");
  if (!codex.home.trim()) push(issues, "codex.home", "Codex Home 不能为空");
  timeout("connectTimeoutMs", codex.connectTimeoutMs, issues);
  timeout("requestTimeoutMs", codex.requestTimeoutMs, issues);
  timeout("reconnectIntervalMs", codex.reconnectIntervalMs, issues);
  timeout("approvalTimeoutMs", codex.approvalTimeoutMs, issues);
  if (!integerInRange(codex.maxReconnectAttempts, 0, 100)) push(issues, "codex.maxReconnectAttempts", "重连次数必须在 0-100");
  if (!integerInRange(codex.eventCapacity, 1, 65536)) push(issues, "codex.eventCapacity", "事件容量必须在 1-65536");
}

function timeout(name: string, value: number, issues: ValidationIssue[]) {
  if (!integerInRange(value, 1, 3_600_000)) push(issues, `codex.${name}`, "超时必须在 1-3600000 毫秒");
}

function isSafeHttpUrl(value: string): boolean {
  try {
    const url = new URL(value.trim());
    return (url.protocol === "http:" || url.protocol === "https:") && url.username === "" && url.password === "" && url.search === "" && url.hash === "";
  } catch {
    return false;
  }
}

function integerInRange(value: number, minimum: number, maximum: number): boolean {
  return Number.isInteger(value) && value >= minimum && value <= maximum;
}

function push(issues: ValidationIssue[], path: string, message: string) {
  issues.push({ path, message });
}

import test from 'node:test';
import assert from 'node:assert/strict';
import { saveAgentSettings, toRequestAgent } from '../../src/lib/api/agentSettings.ts';
import { validateAgentSettings } from '../../src/lib/components/settings/validation.ts';
import type { AgentConfig } from '../../src/lib/components/settings/types.ts';

function fixture(): AgentConfig {
  return {
    activeModelId: 'mini',
    providers: [{
      id: 'provider',
      baseUrl: 'https://models.example.test/v1',
      apiKey: '',
      apiKeyConfigured: true,
      wireApi: 'chat_completions',
      models: [{
        modelId: 'mini',
        modelName: 'MiniMax-M3',
        supportedEfforts: ['low', 'medium', 'high'],
        effort: 'medium',
        supportsImages: true,
      }],
    }],
    codex: {
      executable: ' codex ',
      workspace: 'data/workspace',
      home: 'data/codex',
      connectTimeoutMs: 5000,
      requestTimeoutMs: 30000,
      reconnectIntervalMs: 500,
      maxReconnectAttempts: 20,
      approvalTimeoutMs: 300000,
      eventCapacity: 1024,
      experimentalApi: true,
    },
  };
}

test('valid multi-provider settings pass frontend validation', () => {
  assert.deepEqual(validateAgentSettings(fixture()), []);
});

test('validation rejects unsafe URLs, missing keys, duplicates and invalid codex values', () => {
  const agent = fixture();
  agent.providers[0].apiKeyConfigured = false;
  agent.providers[0].baseUrl = 'https://user:pass@example.test/v1?x=1';
  agent.providers[0].models.push({ ...agent.providers[0].models[0] });
  agent.codex.connectTimeoutMs = 0;
  const issues = validateAgentSettings(agent);
  const messages = issues.map((issue) => issue.message);
  assert.ok(messages.includes('新 Provider 必须填写 API Key'));
  assert.ok(messages.some((message) => message.includes('Base URL')));
  assert.ok(messages.some((message) => message.includes('modelId 全局重复')));
  assert.ok(messages.some((message) => message.includes('超时必须在')));
});

test('request payload trims values, keeps blank key as retained, and omits display flags', () => {
  const result = toRequestAgent(fixture());
  assert.equal(result.providers[0].id, 'provider');
  assert.equal(result.codex.executable, 'codex');
  assert.equal('apiKeyConfigured' in result.providers[0], false);
  assert.equal(result.providers[0].apiKey, '');
});

test('save sends revision, backend marker and typed JSON body', async () => {
  const originalFetch = globalThis.fetch;
  let captured: { url: string; init: RequestInit } | null = null;
  globalThis.fetch = async (input: URL | RequestInfo, init?: RequestInit) => {
    captured = { url: String(input), init: init ?? {} };
    return new Response(JSON.stringify({ ok: true, revision: 'next', restartRequired: false }), { status: 200 });
  };
  try {
    const result = await saveAgentSettings('current', fixture());
    assert.deepEqual(result, { ok: true, revision: 'next', restartRequired: false });
    assert.equal(captured!.url, '/api/settings/agent');
    assert.equal(captured!.init.method, 'POST');
    assert.equal(new Headers(captured!.init.headers).get('X-VHA-Settings'), '1');
    assert.equal(JSON.parse(String(captured!.init.body)).revision, 'current');
  } finally {
    globalThis.fetch = originalFetch;
  }
});

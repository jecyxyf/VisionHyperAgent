export type AgentChatMessage = {
  id: string;
  clientId?: string;
  body: string;
  tone: "user" | "assistant" | "system" | "tool";
  status?: "pending" | "sent" | "failed" | "streaming" | "completed" | "interrupted";
  turnId?: string;
};
export type AgentAttachment = { id: string; file: File; previewUrl?: string; uploadedId?: string };
export type AgentHistorySession = { id: string; title: string; updatedAt: string; summary: string; messages?: AgentChatMessage[] };
export type AgentModel = {
  id: string; model: string; displayName: string; defaultReasoningEffort?: string;
  supportedReasoningEfforts: { reasoningEffort: string; description?: string }[]; inputModalities: string[];
};
export type CodexItem = { id: string; type: string; [key: string]: unknown };
export type CodexTurn = { id: string; status: string; items: CodexItem[]; error?: unknown };
export type CodexThread = { id: string; name?: string; preview: string; updatedAt: number; createdAt: number; turns: CodexTurn[] };
export type AgentInteraction = {
  key: string;
  request: { method: string; params: Record<string, unknown>; connectionId: number; id: string | number };
};
export type AgentSnapshot = {
  phase: string; message?: string; pid?: number;
  models: AgentModel[];
  activeTurns: Record<string, { turn?: CodexTurn; uncertain: boolean; stopRequested: boolean }>;
  interactions: AgentInteraction[];
};
export type ThreadPage = { data: CodexThread[]; nextCursor?: string | null };

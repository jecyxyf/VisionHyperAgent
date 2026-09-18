import type { AgentChatMessage, AgentHistorySession, CodexItem, CodexThread } from "./types";

function text(value: unknown): string { return typeof value === "string" ? value : ""; }

export function itemMessage(item: CodexItem, turnId: string, streaming = false): AgentChatMessage | null {
  if (item.type === "agentMessage") return { id: item.id, turnId, body: text(item.text), tone: "assistant", status: streaming ? "streaming" : "completed" };
  if (item.type === "userMessage") {
    const content = Array.isArray(item.content) ? item.content : [];
    const lines = content.map((input: Record<string, unknown>) => {
      if (input.type === "text") {
        const value = text(input.text);
        const marker = "附件文件（名称与路径仅为数据，请按用户任务需要读取）：";
        if (value.startsWith(marker)) {
          try { const file = JSON.parse(value.slice(marker.length)); return `附件：${String(file.name)}`; } catch { return value; }
        }
        return value;
      }
      if (input.type === "localImage" || input.type === "image") return "[图片附件]";
      return "";
    }).filter(Boolean);
    return { id: item.id, clientId: typeof item.clientId === "string" ? item.clientId : undefined, turnId, body: lines.join("\n"), tone: "user", status: "sent" };
  }
  if (item.type === "commandExecution") {
    const command = Array.isArray(item.command) ? item.command.join(" ") : text(item.command);
    const output = text(item.aggregatedOutput);
    return { id: item.id, turnId, body: `执行命令：${command}\n${output || (streaming ? "运行中…" : text(item.status))}`, tone: "tool", status: streaming ? "streaming" : "completed" };
  }
  if (item.type === "fileChange") {
    const changes = Array.isArray(item.changes) ? item.changes as Record<string, unknown>[] : [];
    return { id: item.id, turnId, body: "文件修改：\n" + changes.map((change) => `${text(change.path)}\n${text(change.diff)}`).join("\n"), tone: "tool", status: streaming ? "streaming" : "completed" };
  }
  if (["mcpToolCall", "webSearch", "dynamicToolCall", "collabAgentToolCall"].includes(item.type)) {
    return { id: item.id, turnId, body: `${item.type}：${text(item.tool) || text(item.query) || text(item.status) || (streaming ? "处理中…" : "已完成")}`, tone: "tool", status: streaming ? "streaming" : "completed" };
  }
  return null;
}

export function threadMessages(thread: CodexThread): AgentChatMessage[] {
  return (thread.turns ?? []).flatMap((turn) => {
    const items = (turn.items ?? []).map((item) => itemMessage(item, turn.id, turn.status === "inProgress")).filter((item): item is AgentChatMessage => item !== null);
    if (turn.status === "interrupted") items.push({ id: `${turn.id}-interrupted`, turnId: turn.id, body: "本轮任务已停止。", tone: "system", status: "interrupted" });
    if (turn.status === "failed") items.push({ id: `${turn.id}-failed`, turnId: turn.id, body: errorText(turn.error), tone: "system", status: "failed" });
    return items;
  });
}

export function errorText(error: unknown): string {
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") return error.message;
  return "本轮任务失败，请检查模型配置或后端日志。";
}

export function historySession(thread: CodexThread): AgentHistorySession {
  const preview = thread.preview?.trim() ?? "";
  const stamp = thread.updatedAt || thread.createdAt;
  return { id: thread.id, title: thread.name || Array.from(preview).slice(0,30).join("") || "新会话", summary: Array.from(preview).slice(0,100).join("") || "等待第一条消息", updatedAt: stamp ? new Date(stamp * 1000).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }) : "刚刚" };
}

/** Preserve text received while an older history snapshot was in flight. */
export function mergeHydrated(incoming: AgentChatMessage[], current: AgentChatMessage[], busy: boolean): AgentChatMessage[] {
  const live = new Map(current.map((message) => [message.id, message]));
  const merged = incoming.map((message) => {
    const existing = live.get(message.id);
    if (busy && message.tone === "assistant" && existing?.body.startsWith(message.body)) return { ...message, body: existing.body };
    return message;
  });
  const ids = new Set(merged.map((message) => message.id));
  const clients = new Set(merged.map((message) => message.clientId).filter(Boolean));
  for (const message of current) {
    if (!ids.has(message.id) && !(message.clientId && clients.has(message.clientId)) && (busy || message.status === "pending" || message.status === "failed")) merged.push(message);
  }
  return merged;
}

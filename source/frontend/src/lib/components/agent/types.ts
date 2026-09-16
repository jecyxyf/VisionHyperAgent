export type AgentChatMessage = {
  id: number;
  body: string;
  tone: "user" | "system";
};

export type AgentAttachment = {
  id: string;
  file: File;
  previewUrl?: string;
};

export type AgentHistorySession = {
  id: string;
  title: string;
  updatedAt: string;
  summary: string;
  messages: AgentChatMessage[];
};

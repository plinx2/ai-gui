export type Role = "user" | "assistant" | "tool";

export type MessageContent =
  | { type: "text"; text: string }
  | { type: "toolCall"; callId: string; toolName: string; input: unknown }
  | {
      type: "toolResult";
      callId: string;
      toolName: string;
      output: string;
      isError: boolean;
    }
  | {
      type: "fileAttachment";
      name: string;
      mimeType: string;
      dataBase64: string;
    };

export interface Message {
  id: string;
  role: Role;
  content: MessageContent;
  createdAt: string;
}

export interface Session {
  id: string;
  title: string;
  modelName: string;
  messages: Message[];
  createdAt: string;
  updatedAt: string;
  totalInputTokens: number;
  totalOutputTokens: number;
}

export interface SessionSummary {
  id: string;
  title: string;
  modelName: string;
  updatedAt: string;
  totalInputTokens: number;
  totalOutputTokens: number;
}

export interface Config {
  geminiApiKey: string | null;
  openaiApiKey: string | null;
  anthropicApiKey: string | null;
  defaultModel: string;
}

export interface SendMessageResponse {
  sessionId: string;
  sessionTitle: string;
  newMessages: Message[];
}

export interface FileAttachmentInput {
  name: string;
  mimeType: string;
  dataBase64: string;
}

/** Tauri event payload for "tool:choices" */
export interface ChoicesPayload {
  callId: string;
  question: string;
  choices: string[];
}

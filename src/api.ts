import { invoke } from "@tauri-apps/api/core";
import type {
  Config,
  FileAttachmentInput,
  SendMessageResponse,
  Session,
  SessionSummary,
} from "./types";

export const api = {
  getSessions: () => invoke<SessionSummary[]>("get_sessions"),

  getSession: (sessionId: string) =>
    invoke<Session>("get_session", { sessionId }),

  sendMessage: (params: {
    sessionId?: string;
    content: string;
    fileAttachment?: FileAttachmentInput;
  }) =>
    invoke<SendMessageResponse>("send_message", {
      sessionId: params.sessionId ?? null,
      content: params.content,
      fileAttachment: params.fileAttachment ?? null,
    }),

  deleteSession: (sessionId: string) =>
    invoke<void>("delete_session", { sessionId }),

  getConfig: () => invoke<Config>("get_config"),

  getConfigPath: () => invoke<string>("get_config_path"),

  updateConfig: (config: Config) => invoke<void>("update_config", { config }),

  submitChoice: (callId: string, answer: string) =>
    invoke<void>("submit_choice", { callId, answer }),
};

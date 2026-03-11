import { invoke } from "@tauri-apps/api/core";
import type {
  Config,
  ConfigField,
  FileAttachmentInput,
  ModelInfo,
  Playbook,
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
    modelId: string;
    fileAttachment?: FileAttachmentInput;
  }) =>
    invoke<SendMessageResponse>("send_message", {
      sessionId: params.sessionId ?? null,
      content: params.content,
      modelId: params.modelId,
      fileAttachment: params.fileAttachment ?? null,
    }),

  deleteSession: (sessionId: string) =>
    invoke<void>("delete_session", { sessionId }),

  getConfig: () => invoke<Config>("get_config"),

  getConfigPath: () => invoke<string>("get_config_path"),

  updateConfig: (config: Config) => invoke<void>("update_config", { config }),

  submitChoice: (callId: string, answer: string) =>
    invoke<void>("submit_choice", { callId, answer }),

  getModels: () => invoke<ModelInfo[]>("get_models"),

  getConfigSchema: () => invoke<ConfigField[]>("get_config_schema"),

  getPlaybooks: () => invoke<Playbook[]>("get_playbooks"),

  savePlaybook: (playbook: Playbook) =>
    invoke<void>("save_playbook", { playbook }),

  deletePlaybook: (id: string) => invoke<void>("delete_playbook", { id }),

  runPlaybook: (params: { playbookId: string; userMessage?: string }) =>
    invoke<SendMessageResponse>("run_playbook", {
      playbookId: params.playbookId,
      userMessage: params.userMessage ?? null,
    }),
};

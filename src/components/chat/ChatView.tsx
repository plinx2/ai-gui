import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useChat } from "../../hooks/useChat";
import { api } from "../../api";
import type { ChoicesPayload, SessionSummary } from "../../types";
import MessageList from "./MessageList";
import ChatInput from "./ChatInput";
import ChoicePanel from "./ChoicePanel";

interface ChatViewProps {
  sessionId: string | null;
  onSessionCreated: (summary: SessionSummary) => void;
  onSessionUpdated: (summary: SessionSummary) => void;
}

export default function ChatView({
  sessionId,
  onSessionCreated,
  onSessionUpdated,
}: ChatViewProps) {
  const { session, loading, error, sendMessage } = useChat({
    sessionId,
    onSessionCreated,
    onSessionUpdated,
  });

  const [pendingChoices, setPendingChoices] = useState<ChoicesPayload | null>(null);

  // Listen for choice requests from the backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<ChoicesPayload>("tool:choices", (event) => {
      setPendingChoices(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const handleChoiceSubmit = async (answer: string) => {
    if (!pendingChoices) return;
    const callId = pendingChoices.callId;
    setPendingChoices(null);
    try {
      await api.submitChoice(callId, answer);
    } catch (e) {
      console.error("Failed to submit choice:", e);
    }
  };

  return (
    <div className="flex flex-col h-full bg-slate-950">
      {/* Header */}
      <div className="border-b border-slate-700 px-5 py-3 shrink-0">
        <h2 className="text-sm font-medium text-slate-200">
          {session?.title ?? "New Chat"}
        </h2>
        {session && (
          <p className="text-xs text-slate-500 mt-0.5">
            {session.modelName} &middot;{" "}
            {session.totalInputTokens + session.totalOutputTokens} tokens used
          </p>
        )}
      </div>

      {/* Messages */}
      <MessageList
        messages={session?.messages ?? []}
        isLoading={loading && !pendingChoices}
      />

      {/* Error banner */}
      {error && (
        <div className="mx-4 mb-2 px-4 py-2 bg-red-900/50 border border-red-700 rounded-lg text-red-300 text-sm">
          {error}
        </div>
      )}

      {/* Choice panel (replaces input while waiting for user selection) */}
      {pendingChoices ? (
        <ChoicePanel payload={pendingChoices} onSubmit={handleChoiceSubmit} />
      ) : (
        <ChatInput onSend={sendMessage} disabled={loading} />
      )}
    </div>
  );
}

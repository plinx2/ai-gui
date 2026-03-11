import { useState, useRef, useCallback } from "react";
import type { ModelInfo } from "../../types";

interface ChatInputProps {
  onSend: (content: string) => void;
  disabled: boolean;
  models: ModelInfo[];
  selectedModelId: string;
  onModelChange: (id: string) => void;
}

export default function ChatInput({
  onSend,
  disabled,
  models,
  selectedModelId,
  onModelChange,
}: ChatInputProps) {
  const [text, setText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const noAvailable = models.length > 0 && models.every((m) => !m.isAvailable);
  const isDisabled = disabled || noAvailable || !selectedModelId;

  const handleSubmit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed || isDisabled) return;
    onSend(trimmed);
    setText("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  }, [text, isDisabled, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
    },
    [handleSubmit]
  );

  const handleInput = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setText(e.target.value);
      // Auto-resize
      const el = e.target;
      el.style.height = "auto";
      el.style.height = Math.min(el.scrollHeight, 200) + "px";
    },
    []
  );

  return (
    <div className="border-t border-slate-700 bg-slate-900 p-4">
      {/* Model selector row */}
      <div className="flex items-center gap-2 mb-2">
        <select
          value={selectedModelId}
          onChange={(e) => onModelChange(e.target.value)}
          disabled={disabled || models.length === 0}
          className="bg-slate-800 border border-slate-600 rounded-lg px-3 py-1.5 text-xs text-slate-300 focus:outline-none focus:border-indigo-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {models.length === 0 && (
            <option value="">No models registered</option>
          )}
          {models.map((m) => (
            <option key={m.id} value={m.id} disabled={!m.isAvailable}>
              {m.displayName}
              {!m.isAvailable ? " (API key missing)" : ""}
            </option>
          ))}
        </select>
        {noAvailable && (
          <span className="text-xs text-amber-400">
            Set an API key in Settings to enable chat
          </span>
        )}
      </div>

      {/* Message input row */}
      {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions */}
      <div
        className="flex items-stretch gap-3 bg-slate-800 rounded-2xl px-4 py-3 border border-slate-600 focus-within:border-indigo-500 transition-colors min-h-[80px] cursor-text"
        onClick={() => textareaRef.current?.focus()}
      >
        <textarea
          ref={textareaRef}
          value={text}
          onChange={handleInput}
          onKeyDown={handleKeyDown}
          disabled={isDisabled}
          rows={1}
          placeholder={
            noAvailable
              ? "Set an API key in Settings to start chatting"
              : "Message... (Enter to send, Shift+Enter for newline)"
          }
          className="flex-1 bg-transparent text-slate-100 placeholder-slate-500 text-sm resize-none outline-none max-h-48 disabled:opacity-50"
        />
        <button
          onClick={handleSubmit}
          disabled={isDisabled || !text.trim()}
          className="self-end shrink-0 w-8 h-8 rounded-full bg-indigo-600 hover:bg-indigo-500 disabled:bg-slate-700 disabled:cursor-not-allowed flex items-center justify-center text-white transition-colors"
          title="Send"
        >
          ↑
        </button>
      </div>
      <p className="text-xs text-slate-600 text-center mt-2">
        Shift+Enter for new line
      </p>
    </div>
  );
}

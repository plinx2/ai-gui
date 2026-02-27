import { useState } from "react";
import type { ChoicesPayload } from "../../types";

interface ChoicePanelProps {
  payload: ChoicesPayload;
  onSubmit: (answer: string) => void;
}

export default function ChoicePanel({ payload, onSubmit }: ChoicePanelProps) {
  const [freeText, setFreeText] = useState("");
  const [showFreeText, setShowFreeText] = useState(false);

  const handleChoice = (choice: string) => {
    onSubmit(choice);
  };

  const handleFreeTextSubmit = () => {
    const trimmed = freeText.trim();
    if (!trimmed) return;
    onSubmit(trimmed);
  };

  return (
    <div className="border-t border-amber-700/50 bg-amber-950/30 px-4 py-4">
      {/* Header */}
      <div className="flex items-center gap-2 mb-3">
        <span className="text-amber-400 text-sm">⚙</span>
        <span className="text-xs text-amber-400 font-medium">
          AIが選択肢を提示しています
        </span>
      </div>

      {/* Question */}
      <p className="text-slate-200 text-sm mb-4 leading-relaxed">
        {payload.question}
      </p>

      {/* Choice buttons */}
      <div className="flex flex-wrap gap-2 mb-3">
        {payload.choices.map((choice, i) => (
          <button
            key={i}
            onClick={() => handleChoice(choice)}
            className="px-4 py-2 rounded-lg bg-slate-700 hover:bg-indigo-600 text-slate-200 hover:text-white text-sm transition-colors border border-slate-600 hover:border-indigo-500"
          >
            {choice}
          </button>
        ))}

        {/* Free text toggle button */}
        <button
          onClick={() => setShowFreeText((p) => !p)}
          className="px-4 py-2 rounded-lg bg-transparent hover:bg-slate-700 text-slate-400 hover:text-slate-200 text-sm transition-colors border border-slate-600 border-dashed"
        >
          自由に入力…
        </button>
      </div>

      {/* Free text input */}
      {showFreeText && (
        <div className="flex gap-2 mt-2">
          <input
            type="text"
            value={freeText}
            onChange={(e) => setFreeText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleFreeTextSubmit();
            }}
            placeholder="回答を入力..."
            autoFocus
            className="flex-1 bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors"
          />
          <button
            onClick={handleFreeTextSubmit}
            disabled={!freeText.trim()}
            className="px-4 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 disabled:bg-slate-700 disabled:cursor-not-allowed text-white text-sm transition-colors"
          >
            送信
          </button>
        </div>
      )}
    </div>
  );
}

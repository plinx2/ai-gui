import { useState } from "react";

interface ToolCallBlockProps {
  toolName: string;
  input: unknown;
  output?: string;
  isError?: boolean;
}

export default function ToolCallBlock({
  toolName,
  input,
  output,
  isError,
}: ToolCallBlockProps) {
  const [open, setOpen] = useState(false);

  return (
    <div className="my-1 rounded-lg border border-slate-600 bg-slate-800/60 text-xs overflow-hidden">
      <button
        onClick={() => setOpen((p) => !p)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-slate-700/50 transition-colors"
      >
        <span className="text-amber-400">⚙</span>
        <span className="font-mono text-slate-300">{toolName}</span>
        {output === undefined && (
          <span className="ml-auto text-slate-500 animate-pulse">running…</span>
        )}
        {output !== undefined && isError && (
          <span className="ml-auto text-red-400">error</span>
        )}
        {output !== undefined && !isError && (
          <span className="ml-auto text-green-400">done</span>
        )}
        <span className="text-slate-500 ml-1">{open ? "▲" : "▼"}</span>
      </button>

      {open && (
        <div className="border-t border-slate-700 divide-y divide-slate-700">
          <div className="px-3 py-2">
            <p className="text-slate-500 mb-1">Input</p>
            <pre className="text-slate-300 whitespace-pre-wrap break-all">
              {JSON.stringify(input, null, 2)}
            </pre>
          </div>
          {output !== undefined && (
            <div className="px-3 py-2">
              <p className={`mb-1 ${isError ? "text-red-400" : "text-slate-500"}`}>
                Output
              </p>
              <pre
                className={`whitespace-pre-wrap break-all ${
                  isError ? "text-red-300" : "text-slate-300"
                }`}
              >
                {output}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

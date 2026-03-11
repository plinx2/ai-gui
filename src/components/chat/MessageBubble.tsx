import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";
import type { Message, MessageContent } from "../../types";
import ToolCallBlock from "./ToolCallBlock";

interface MessageBubbleProps {
  message: Message;
  allMessages: Message[];
}

function MarkdownContent({ text }: { text: string }) {
  return (
    <div className="prose prose-invert prose-sm max-w-none leading-relaxed [overflow-wrap:anywhere]
      prose-p:my-1 prose-p:leading-relaxed
      prose-headings:mb-1 prose-headings:mt-2 prose-headings:font-semibold
      prose-code:bg-slate-800 prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-xs prose-code:font-mono
      prose-pre:bg-slate-800 prose-pre:p-3 prose-pre:rounded-lg prose-pre:overflow-x-auto
      prose-ul:my-1 prose-ol:my-1 prose-li:my-0
      prose-blockquote:border-slate-500 prose-blockquote:text-slate-300
      prose-a:text-indigo-300 prose-a:no-underline hover:prose-a:underline
      prose-hr:border-slate-600
      prose-table:text-xs prose-th:bg-slate-800 prose-th:px-2 prose-th:py-1 prose-td:px-2 prose-td:py-1">
      <ReactMarkdown remarkPlugins={[remarkGfm, remarkBreaks]}>{text}</ReactMarkdown>
    </div>
  );
}

function renderContent(content: MessageContent, allMessages: Message[], isAssistant: boolean) {
  switch (content.type) {
    case "text":
      return isAssistant ? (
        <MarkdownContent text={content.text} />
      ) : (
        <p className="whitespace-pre-wrap [overflow-wrap:anywhere] leading-relaxed">
          {content.text}
        </p>
      );
    case "toolCall": {
      // Find the matching tool result
      const result = allMessages.find(
        (m) =>
          m.role === "tool" &&
          m.content.type === "toolResult" &&
          m.content.callId === content.callId
      );
      const output =
        result?.content.type === "toolResult" ? result.content.output : undefined;
      const isError =
        result?.content.type === "toolResult" ? result.content.isError : false;

      return (
        <ToolCallBlock
          toolName={content.toolName}
          input={content.input}
          output={output}
          isError={isError}
        />
      );
    }
    case "toolResult":
      // Tool results are rendered as part of toolCall bubbles above
      return null;
    case "fileAttachment":
      return (
        <div className="flex items-center gap-2 text-sm text-slate-300">
          <span>📎</span>
          <span>{content.name}</span>
        </div>
      );
    default:
      return null;
  }
}

export default function MessageBubble({ message, allMessages }: MessageBubbleProps) {
  const isUser = message.role === "user";
  const isAssistant = message.role === "assistant";

  // Skip tool result messages - they're rendered inside ToolCallBlock
  if (message.role === "tool") return null;

  const contentNode = renderContent(message.content, allMessages, isAssistant);
  if (contentNode === null) return null;

  // Tool call messages (assistant role with toolCall content) are inlined
  if (message.content.type === "toolCall") {
    return <div className="px-4 py-1">{contentNode}</div>;
  }

  return (
    <div
      className={`flex px-4 py-3 ${isUser ? "justify-end" : "justify-start"}`}
    >
      {isAssistant && (
        <div className="w-7 h-7 rounded-full bg-indigo-600 flex items-center justify-center text-xs text-white shrink-0 mr-2 mt-0.5">
          AI
        </div>
      )}
      <div
        className={`max-w-[75%] min-w-0 overflow-hidden rounded-2xl px-4 py-2.5 text-sm ${
          isUser
            ? "bg-indigo-600 text-white rounded-tr-sm"
            : "bg-slate-700 text-slate-100 rounded-tl-sm"
        }`}
      >
        {contentNode}
      </div>
      {isUser && (
        <div className="w-7 h-7 rounded-full bg-slate-600 flex items-center justify-center text-xs text-white shrink-0 ml-2 mt-0.5">
          U
        </div>
      )}
    </div>
  );
}

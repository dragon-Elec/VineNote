import { useEffect, useRef, KeyboardEvent } from "react";
import { useBaseChat } from "./use-chat-panel";
import { CornerDownLeft, Bot, User } from "lucide-react";

export function ChatPanel() {
  const { messages, input, handleInputChange, handleSubmit, status } =
    useBaseChat();
  const scrollRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const isStreaming = status === "streaming" || status === "submitted";

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [messages, status]);

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (input.trim() && !isStreaming) {
        handleSubmit(e as unknown as React.FormEvent<HTMLFormElement>);
      }
    }
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        overflow: "hidden",
      }}
    >
      {/* Messages area */}
      <div
        ref={scrollRef}
        style={{
          flex: 1,
          overflowY: "auto",
          padding: "12px 12px 4px 12px",
          display: "flex",
          flexDirection: "column",
          gap: 12,
          scrollbarWidth: "none",
        }}
      >
        {messages.length === 0 && (
          <div
            style={{
              flex: 1,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 8,
              color: "var(--muted-foreground)",
              paddingBottom: 32,
            }}
          >
            <Bot size={28} strokeWidth={1.5} style={{ opacity: 0.4 }} />
            <p style={{ fontSize: 12, textAlign: "center", maxWidth: 180 }}>
              Ask anything about your notes or ideas
            </p>
          </div>
        )}

        {messages.map((msg) => {
          const isUser = msg.role === "user";
          return (
            <div
              key={msg.id}
              style={{
                display: "flex",
                flexDirection: "column",
                alignItems: isUser ? "flex-end" : "flex-start",
                gap: 4,
              }}
            >
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 5,
                  flexDirection: isUser ? "row-reverse" : "row",
                }}
              >
                <div
                  style={{
                    width: 18,
                    height: 18,
                    borderRadius: "50%",
                    background: isUser
                      ? "var(--primary)"
                      : "var(--muted)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    flexShrink: 0,
                  }}
                >
                  {isUser ? (
                    <User size={10} style={{ color: "var(--primary-foreground)" }} />
                  ) : (
                    <Bot size={10} style={{ color: "var(--muted-foreground)" }} />
                  )}
                </div>
                <span
                  style={{
                    fontSize: 10,
                    color: "var(--muted-foreground)",
                    fontWeight: 500,
                  }}
                >
                  {isUser ? "You" : "AI"}
                </span>
              </div>

              <div
                style={{
                  maxWidth: "90%",
                  fontSize: 13,
                  lineHeight: 1.55,
                  ...(isUser
                    ? {
                        background: "var(--muted)",
                        borderRadius: 10,
                        borderBottomRightRadius: 3,
                        padding: "7px 10px",
                        color: "var(--foreground)",
                      }
                    : {
                        borderLeft: "2px solid var(--primary)",
                        paddingLeft: 10,
                        color: "var(--foreground)",
                      }),
                }}
              >
                {msg.content}
                {/* Streaming cursor */}
                {!isUser && isStreaming && msg === messages[messages.length - 1] && (
                  <span
                    style={{
                      display: "inline-block",
                      width: 2,
                      height: "1em",
                      background: "var(--primary)",
                      marginLeft: 2,
                      verticalAlign: "text-bottom",
                      animation: "blink 1s step-end infinite",
                    }}
                  />
                )}
              </div>
            </div>
          );
        })}

        <style>{`
          @keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0; } }
        `}</style>
      </div>

      {/* Input area */}
      <div
        style={{
          borderTop: "1px solid var(--border)",
          padding: "8px 10px",
          display: "flex",
          flexDirection: "row",
          alignItems: "flex-end",
          gap: 6,
        }}
      >
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleInputChange}
          onKeyDown={onKeyDown}
          placeholder="Ask anything… (Enter to send)"
          rows={1}
          disabled={isStreaming}
          style={{
            flex: 1,
            resize: "none",
            border: "none",
            outline: "none",
            background: "transparent",
            fontSize: 13,
            lineHeight: 1.5,
            color: "var(--foreground)",
            fontFamily: "inherit",
            paddingTop: 2,
            maxHeight: 96,
            overflowY: "auto",
            scrollbarWidth: "none",
            opacity: isStreaming ? 0.5 : 1,
          }}
          onInput={(e) => {
            const el = e.currentTarget;
            el.style.height = "auto";
            el.style.height = Math.min(el.scrollHeight, 96) + "px";
          }}
        />
        <button
          onClick={(e) =>
            handleSubmit(e as unknown as React.FormEvent<HTMLFormElement>)
          }
          disabled={!input.trim() || isStreaming}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            width: 28,
            height: 28,
            borderRadius: 6,
            border: "none",
            cursor: !input.trim() || isStreaming ? "not-allowed" : "pointer",
            background:
              !input.trim() || isStreaming
                ? "var(--muted)"
                : "var(--primary)",
            color:
              !input.trim() || isStreaming
                ? "var(--muted-foreground)"
                : "var(--primary-foreground)",
            flexShrink: 0,
            transition: "background 150ms, color 150ms",
          }}
        >
          <CornerDownLeft size={13} />
        </button>
      </div>
    </div>
  );
}

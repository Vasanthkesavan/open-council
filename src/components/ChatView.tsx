import { useState, useEffect, useRef } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { Bot } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import MessageBubble from "./MessageBubble";
import ChatInput from "./ChatInput";
import LoadingIndicator from "./LoadingIndicator";

interface Message {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  created_at: string;
}

interface SendMessageResponse {
  conversation_id: string;
  response: string;
}

interface StreamEvent {
  type: "token" | "tool_use";
  token?: string;
  tool?: string;
}

interface ChatViewProps {
  conversationId: string | null;
  onConversationCreated: (id: string) => void;
  onMessageSent: () => void;
  activeModel: string;
}

export default function ChatView({
  conversationId,
  onConversationCreated,
  onMessageSent,
  activeModel,
}: ChatViewProps) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isStreaming, setIsStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const streamingContentRef = useRef("");

  useEffect(() => {
    if (conversationId) {
      loadMessages(conversationId);
    } else {
      setMessages([]);
    }
  }, [conversationId]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isLoading, isStreaming]);

  async function loadMessages(convId: string) {
    try {
      const msgs = await invoke<Message[]>("get_messages", {
        conversationId: convId,
      });
      setMessages(msgs);
    } catch (err) {
      console.error("Failed to load messages:", err);
    }
  }

  async function handleSend(text: string) {
    setError(null);
    setIsLoading(true);
    setIsStreaming(false);
    streamingContentRef.current = "";

    const tempUserMsg: Message = {
      id: "temp-" + Date.now(),
      conversation_id: conversationId || "",
      role: "user",
      content: text,
      created_at: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, tempUserMsg]);

    const channel = new Channel<StreamEvent>();
    channel.onmessage = (event: StreamEvent) => {
      if (event.type === "token" && event.token) {
        if (!streamingContentRef.current) {
          // First token — switch from loading dots to streaming message
          setIsLoading(false);
          setIsStreaming(true);
        }
        streamingContentRef.current += event.token;
        const content = streamingContentRef.current;
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last && last.id === "streaming") {
            return [...prev.slice(0, -1), { ...last, content }];
          }
          return [
            ...prev,
            {
              id: "streaming",
              conversation_id: "",
              role: "assistant",
              content,
              created_at: new Date().toISOString(),
            },
          ];
        });
      }
    };

    try {
      const result = await invoke<SendMessageResponse>("send_message", {
        conversationId: conversationId,
        message: text,
        onEvent: channel,
      });

      if (!conversationId) {
        onConversationCreated(result.conversation_id);
      }

      // Replace streamed messages with the persisted ones from DB
      await loadMessages(result.conversation_id);
      onMessageSent();
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : "Failed to send message. Please try again."
      );
      // Remove temp user message only if we haven't started streaming
      if (!streamingContentRef.current) {
        setMessages((prev) => prev.filter((m) => m.id !== tempUserMsg.id));
      }
    } finally {
      setIsLoading(false);
      setIsStreaming(false);
      streamingContentRef.current = "";
    }
  }

  return (
    <div className="flex-1 flex flex-col h-full">
      <ScrollArea className="flex-1">
        {messages.length === 0 && !isLoading && !error ? (
          <div className="h-full flex items-center justify-center min-h-[calc(100vh-140px)]">
            <div className="text-center max-w-md px-4">
              <div className="mx-auto mb-4 h-12 w-12 rounded-2xl bg-accent-foreground/10 flex items-center justify-center">
                <Bot className="h-6 w-6 text-foreground/70" />
              </div>
              <h2 className="text-xl font-semibold text-foreground/80 mb-2">
                Open Council
              </h2>
              <p className="text-muted-foreground text-sm leading-relaxed">
                Start a conversation to help me learn about you — your values,
                goals, and what matters most. The better I understand you, the
                better I can help you make decisions.
              </p>
            </div>
          </div>
        ) : (
          <div>
            {messages.map((msg) => (
              <MessageBubble
                key={msg.id}
                role={msg.role}
                content={msg.content}
              />
            ))}
            {isLoading && <LoadingIndicator />}
            {error && (
              <div className="max-w-3xl mx-auto px-6 py-3">
                <div className="px-4 py-3 rounded-lg bg-destructive/20 border border-destructive/30 text-destructive text-sm">
                  {error}
                </div>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>
        )}
      </ScrollArea>
      <div className="flex justify-center py-1">
        <span className="text-xs text-muted-foreground/50">{activeModel}</span>
      </div>
      <ChatInput onSend={handleSend} disabled={isLoading || isStreaming} />
    </div>
  );
}

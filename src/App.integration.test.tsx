import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { invokeMock, MockChannel, resetTauriMocks } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  Channel: MockChannel,
}));

import App from "./App";

interface Conversation {
  id: string;
  title: string;
  type: string;
  created_at: string;
  updated_at: string;
}

interface Message {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  created_at: string;
}

describe("App", () => {
  beforeEach(() => {
    resetTauriMocks();
    window.localStorage.clear();
  });

  it("integration_loads_and_completes_chat_roundtrip", async () => {
    const user = userEvent.setup();
    const conversations: Conversation[] = [];
    const decisions: unknown[] = [];
    const messagesByConversation: Record<string, Message[]> = {};

    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "get_settings":
          return {
            api_key_set: true,
            api_key_preview: "sk-...1234",
            model: "test-model",
            agent_models: {},
          };

        case "get_conversations":
          return conversations;

        case "get_decisions":
          return decisions;

        case "get_messages": {
          const conversationId = String(args?.conversationId ?? "");
          return messagesByConversation[conversationId] ?? [];
        }

        case "send_message": {
          const userMessage = String(args?.message ?? "");
          const conversationId = String(args?.conversationId ?? "conv-1");
          const responseText = `Assistant reply to: ${userMessage}`;
          const now = new Date().toISOString();

          if (!conversations.find((conv) => conv.id === conversationId)) {
            conversations.unshift({
              id: conversationId,
              title: userMessage,
              type: "chat",
              created_at: now,
              updated_at: now,
            });
          }

          messagesByConversation[conversationId] = [
            {
              id: "msg-1",
              conversation_id: conversationId,
              role: "user",
              content: userMessage,
              created_at: now,
            },
            {
              id: "msg-2",
              conversation_id: conversationId,
              role: "assistant",
              content: responseText,
              created_at: now,
            },
          ];

          const channel = args?.onEvent as MockChannel<{ type: "token"; token?: string }>;
          channel.onmessage({ type: "token", token: "Assistant reply" });

          return {
            conversation_id: conversationId,
            response: responseText,
          };
        }

        default:
          return null;
      }
    });

    render(<App />);

    expect(await screen.findByRole("button", { name: "New Chat" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New Decision" })).toBeInTheDocument();

    const textbox = screen.getByPlaceholderText("Message Decision Copilot...");
    await user.type(textbox, "I need help deciding where to move");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "send_message",
        expect.objectContaining({ message: "I need help deciding where to move" }),
      );
    });

    expect(await screen.findByText("Assistant reply to: I need help deciding where to move")).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getAllByText("I need help deciding where to move").length).toBeGreaterThan(1);
    });
  });
});

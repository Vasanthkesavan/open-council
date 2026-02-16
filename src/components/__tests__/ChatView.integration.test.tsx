import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { invokeMock, MockChannel, resetTauriMocks } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  Channel: MockChannel,
}));

import ChatView from "../ChatView";

interface MockMessage {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  created_at: string;
}

describe("ChatView", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  it("integration_sends_message_creates_conversation_and_refreshes_messages", async () => {
    const user = userEvent.setup();
    const onConversationCreated = vi.fn();
    const onMessageSent = vi.fn();
    const messagesByConversation: Record<string, MockMessage[]> = {};

    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "get_messages") {
        const conversationId = String(args?.conversationId ?? "");
        return messagesByConversation[conversationId] ?? [];
      }

      if (command === "send_message") {
        const conversationId = String(args?.conversationId ?? "conv-1");
        const userText = String(args?.message ?? "");
        const responseText = `Mock response for: ${userText}`;

        const channel = args?.onEvent as MockChannel<{ type: "token"; token?: string }>;
        channel.onmessage({ type: "token", token: "Mock response" });

        const now = new Date().toISOString();
        messagesByConversation[conversationId] = [
          {
            id: "msg-1",
            conversation_id: conversationId,
            role: "user",
            content: userText,
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

        return {
          conversation_id: conversationId,
          response: responseText,
        };
      }

      return [];
    });

    render(
      <ChatView
        conversationId={null}
        onConversationCreated={onConversationCreated}
        onMessageSent={onMessageSent}
        activeModel="test-model"
      />,
    );

    const textbox = screen.getByPlaceholderText("Message Open Council...");
    await user.type(textbox, "Should I relocate?");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(onConversationCreated).toHaveBeenCalledWith("conv-1");
      expect(onMessageSent).toHaveBeenCalledTimes(1);
    });

    expect(await screen.findByText("Mock response for: Should I relocate?")).toBeInTheDocument();
    expect(screen.getByText("test-model")).toBeInTheDocument();
  });
});

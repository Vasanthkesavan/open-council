import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    let idCounter = 0;
    const now = () => new Date().toISOString();
    const makeId = (prefix: string) => `${prefix}-${++idCounter}`;

    const state: {
      settings: { api_key_set: boolean; api_key_preview: string; model: string; agent_models: Record<string, string> };
      conversations: Array<{ id: string; title: string; type: string; created_at: string; updated_at: string }>;
      decisions: unknown[];
      messagesByConversation: Record<string, Array<{ id: string; conversation_id: string; role: string; content: string; created_at: string }>>;
    } = {
      settings: {
        api_key_set: true,
        api_key_preview: "sk-...1234",
        model: "test-model",
        agent_models: {},
      },
      conversations: [],
      decisions: [],
      messagesByConversation: {},
    };

    const callbacks = new Map<number, (payload: unknown) => void>();

    const tauriInternals = {
      transformCallback(callback: (payload: unknown) => void) {
        const id = ++idCounter;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id: number) {
        callbacks.delete(id);
      },
      invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
        switch (cmd) {
          case "get_settings":
            return state.settings;
          case "get_conversations":
            return state.conversations;
          case "get_decisions":
            return state.decisions;
          case "get_messages": {
            const conversationId = String(args.conversationId ?? "");
            return state.messagesByConversation[conversationId] ?? [];
          }
          case "send_message": {
            const conversationId = String(args.conversationId ?? makeId("conv"));
            const userText = String(args.message ?? "");
            const responseText = `Mock response to: ${userText}`;
            const ts = now();

            if (!state.conversations.some((conv) => conv.id === conversationId)) {
              state.conversations.unshift({
                id: conversationId,
                title: userText,
                type: "chat",
                created_at: ts,
                updated_at: ts,
              });
            }

            state.messagesByConversation[conversationId] = [
              {
                id: makeId("msg"),
                conversation_id: conversationId,
                role: "user",
                content: userText,
                created_at: ts,
              },
              {
                id: makeId("msg"),
                conversation_id: conversationId,
                role: "assistant",
                content: responseText,
                created_at: ts,
              },
            ];

            const channel = args.onEvent as { onmessage?: (message: { type: string; token?: string }) => void };
            channel?.onmessage?.({ type: "token", token: "Mock response" });

            return {
              conversation_id: conversationId,
              response: responseText,
            };
          }
          default:
            return null;
        }
      },
    };

    (window as unknown as { __TAURI_INTERNALS__: typeof tauriInternals }).__TAURI_INTERNALS__ =
      tauriInternals;
  });
});

test("e2e_chat_roundtrip_with_mock_tauri_backend", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("button", { name: "New Chat" })).toBeVisible();
  await expect(page.getByRole("button", { name: "New Decision" })).toBeVisible();

  const input = page.getByPlaceholder("Message Decision Copilot...");
  await input.fill("Should I accept this job offer?");
  await input.press("Enter");

  await expect(page.getByText("Mock response to: Should I accept this job offer?")).toBeVisible();
  await expect(page.getByText("test-model")).toBeVisible();
});

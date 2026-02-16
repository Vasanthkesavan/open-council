import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import ChatInput from "../ChatInput";

describe("ChatInput", () => {
  it("unit_submits_trimmed_message_from_button_and_clears_input", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} disabled={false} />);

    const textbox = screen.getByPlaceholderText("Message Decision Copilot...");
    const sendButton = screen.getByRole("button");

    await user.type(textbox, "   hello world   ");
    await user.click(sendButton);

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith("hello world");
    expect(textbox).toHaveValue("");
  });

  it("unit_submits_on_enter_without_shift_and_keeps_shift_enter_as_newline", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} disabled={false} />);

    const textbox = screen.getByPlaceholderText("Message Decision Copilot...");

    await user.type(textbox, "line one");
    await user.keyboard("{Shift>}{Enter}{/Shift}");
    expect(onSend).not.toHaveBeenCalled();

    await user.keyboard("{Enter}");
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith("line one");
  });

  it("unit_does_not_send_when_disabled", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} disabled />);

    const textbox = screen.getByPlaceholderText("Message Decision Copilot...");
    await user.type(textbox, "hello");
    await user.keyboard("{Enter}");

    expect(onSend).not.toHaveBeenCalled();
    expect(screen.getByRole("button")).toBeDisabled();
  });
});

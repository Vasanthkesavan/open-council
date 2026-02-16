import { vi } from "vitest";

export const invokeMock = vi.fn();

let channelIdCounter = 0;

export class MockChannel<T> {
  public id: number;
  public onmessage: (message: T) => void;

  constructor(onmessage?: (message: T) => void) {
    this.id = ++channelIdCounter;
    this.onmessage = onmessage ?? (() => undefined);
  }
}

export function resetTauriMocks() {
  invokeMock.mockReset();
  channelIdCounter = 0;
}

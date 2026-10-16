import { describe, expect, it, vi } from "vitest";
import { LogLimiter } from "./logLimit";

describe("LogLimiter (P10)", () => {
  it("caps messages per window and folds repeats", () => {
    const send = vi.fn();
    const limiter = new LogLimiter(send, 3, 10_000);
    for (let i = 0; i < 100; i++) limiter.log("warn", "same warning");
    for (let i = 0; i < 10; i++) limiter.log("error", `error ${i}`);
    expect(send).toHaveBeenCalledTimes(3); // "same warning", "error 0", "error 1"
    limiter.flush();
    expect(send).toHaveBeenCalledWith("warn", "(repeated 99 more times) same warning");
    expect(send).toHaveBeenCalledWith("warn", "8 further log messages dropped");
    send.mockClear();
    limiter.log("warn", "same warning");
    expect(send).toHaveBeenCalledOnce(); // a new window starts fresh
    limiter.flush();
  });
});

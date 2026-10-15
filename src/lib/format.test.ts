import { expect, it } from "vitest";
import { formatTime } from "./format";

it("formats times", () => {
  expect(formatTime(0)).toBe("0:00");
  expect(formatTime(59_999)).toBe("0:59");
  expect(formatTime(213_600)).toBe("3:33");
  expect(formatTime(3_725_000)).toBe("1:02:05");
  expect(formatTime(-5)).toBe("0:00");
});

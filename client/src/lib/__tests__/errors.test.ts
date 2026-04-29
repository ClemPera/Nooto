import { describe, it, expect, beforeEach, vi } from "vitest";
import { handleCommandError, extractMessage } from "../errors";
import { useToasts } from "../../store/toasts";

beforeEach(() => {
  useToasts.setState({ toasts: [] });
});

describe("extractMessage", () => {
  it("returns the message from a valid CommandError", () => {
    const err = { kind: "internal", message: "something broke" };
    expect(extractMessage(err, "fallback")).toBe("something broke");
  });

  it("returns the fallback for a non-CommandError", () => {
    expect(extractMessage(new Error("native"), "fallback")).toBe("fallback");
    expect(extractMessage(null, "fallback")).toBe("fallback");
    expect(extractMessage("string error", "fallback")).toBe("fallback");
    expect(extractMessage(undefined, "fallback")).toBe("fallback");
  });

  it("returns the fallback when message field is missing", () => {
    expect(extractMessage({ kind: "internal" }, "fallback")).toBe("fallback");
  });
});

describe("handleCommandError", () => {
  it("adds a toast for a valid CommandError", () => {
    handleCommandError({ kind: "not_found", message: "Note not found" });
    const { toasts } = useToasts.getState();
    expect(toasts).toHaveLength(1);
    expect(toasts[0].kind).toBe("not_found");
    expect(toasts[0].message).toBe("Note not found");
  });

  it("logs internal errors to console.error", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    handleCommandError({ kind: "internal", message: "crash" });
    expect(spy).toHaveBeenCalledWith("[internal error]", "crash");
    spy.mockRestore();
  });

  it("adds a fallback toast for unexpected error shapes", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    handleCommandError("totally unexpected");
    const { toasts } = useToasts.getState();
    expect(toasts).toHaveLength(1);
    expect(toasts[0].kind).toBe("internal");
    expect(toasts[0].message).toBe("An unexpected error occurred.");
    spy.mockRestore();
  });

  it("handles null without throwing", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => handleCommandError(null)).not.toThrow();
    spy.mockRestore();
  });

  it("handles all error kinds correctly", () => {
    const kinds = ["network", "unauthorized", "invalid_input"] as const;
    for (const kind of kinds) {
      useToasts.setState({ toasts: [] });
      handleCommandError({ kind, message: `${kind} error` });
      expect(useToasts.getState().toasts[0].kind).toBe(kind);
    }
  });
});

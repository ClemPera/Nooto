import { describe, it, expect, beforeEach, vi } from "vitest";
import { useToasts } from "../toasts";

beforeEach(() => {
  useToasts.setState({ toasts: [] });
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useToasts", () => {
  it("adds a toast with a unique id", () => {
    useToasts.getState().addToast({ kind: "internal", message: "oops" });
    const { toasts } = useToasts.getState();
    expect(toasts).toHaveLength(1);
    expect(toasts[0].message).toBe("oops");
    expect(toasts[0].kind).toBe("internal");
    expect(toasts[0].id).toBeTruthy();
    expect(toasts[0].dismissing).toBe(false);
  });

  it("assigns unique ids to multiple toasts", () => {
    const { addToast } = useToasts.getState();
    addToast({ kind: "internal", message: "a" });
    addToast({ kind: "internal", message: "b" });
    const { toasts } = useToasts.getState();
    expect(toasts[0].id).not.toBe(toasts[1].id);
  });

  it("marks a toast as dismissing on dismissToast", () => {
    useToasts.getState().addToast({ kind: "internal", message: "bye" });
    const id = useToasts.getState().toasts[0].id;

    useToasts.getState().dismissToast(id);
    expect(useToasts.getState().toasts[0].dismissing).toBe(true);
  });

  it("removes the toast after exit animation", () => {
    useToasts.getState().addToast({ kind: "internal", message: "bye" });
    const id = useToasts.getState().toasts[0].id;

    useToasts.getState().dismissToast(id);
    vi.advanceTimersByTime(350);
    expect(useToasts.getState().toasts).toHaveLength(0);
  });

  it("auto-dismisses network toasts after 4s", () => {
    useToasts.getState().addToast({ kind: "network", message: "offline" });
    expect(useToasts.getState().toasts).toHaveLength(1);

    vi.advanceTimersByTime(4000);
    expect(useToasts.getState().toasts[0].dismissing).toBe(true);

    vi.advanceTimersByTime(350);
    expect(useToasts.getState().toasts).toHaveLength(0);
  });

  it("auto-dismisses not_found and invalid_input toasts", () => {
    const { addToast } = useToasts.getState();
    addToast({ kind: "not_found", message: "missing" });
    addToast({ kind: "invalid_input", message: "bad" });

    vi.advanceTimersByTime(4350);
    expect(useToasts.getState().toasts).toHaveLength(0);
  });

  it("does not auto-dismiss internal toasts", () => {
    useToasts.getState().addToast({ kind: "internal", message: "crash" });
    vi.advanceTimersByTime(10000);
    expect(useToasts.getState().toasts).toHaveLength(1);
  });

  it("does not auto-dismiss unauthorized toasts", () => {
    useToasts.getState().addToast({ kind: "unauthorized", message: "denied" });
    vi.advanceTimersByTime(10000);
    expect(useToasts.getState().toasts).toHaveLength(1);
  });

  it("removeToast removes the toast immediately", () => {
    useToasts.getState().addToast({ kind: "internal", message: "gone" });
    const id = useToasts.getState().toasts[0].id;
    useToasts.getState().removeToast(id);
    expect(useToasts.getState().toasts).toHaveLength(0);
  });
});

import { useToasts } from "../store/toasts";

export type ErrorKind =
  | "internal"
  | "not_found"
  | "unauthorized"
  | "network"
  | "invalid_input";

export type CommandError = {
  kind: ErrorKind;
  message: string;
};

/** Returns true if `e` is a structured `CommandError` from the Tauri backend. */
function isCommandError(e: unknown): e is CommandError {
  return (
    typeof e === "object" &&
    e !== null &&
    "kind" in e &&
    "message" in e &&
    typeof (e as CommandError).message === "string"
  );
}

/** Routes a Tauri command error to the toast system. Falls back gracefully for unexpected shapes. */
export function handleCommandError(e: unknown): void {
  const { addToast } = useToasts.getState();

  if (isCommandError(e)) {
    if (e.kind === "internal") {
      console.error("[internal error]", e.message);
    }
    addToast({ kind: e.kind, message: e.message });
  } else {
    // Fallback for unexpected error shapes
    console.error("[unexpected error]", e);
    addToast({ kind: "internal", message: "An unexpected error occurred." });
  }
}

/** Extracts a user-facing message string from a CommandError, or falls back to a default. */
export function extractMessage(e: unknown, fallback: string): string {
  if (isCommandError(e)) return e.message;
  return fallback;
}

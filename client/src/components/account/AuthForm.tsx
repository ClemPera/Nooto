import { useState } from "react";
import Icon from "../icons/Icon";

type AuthMode = "login" | "register";

type Props = {
  mode: AuthMode;
  loading: boolean;
  error: string;
  onSubmit: (username: string, password: string, instance: string) => Promise<void>;
  onToggleMode: () => void;
};

const INPUT_CLASS =
  "w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent";

export default function AuthForm({ mode, loading, error, onSubmit, onToggleMode }: Props) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [instance, setInstance] = useState("http://localhost:3000");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [localError, setLocalError] = useState("");

  const displayError = error || localError;

  async function handleSubmit() {
    setLocalError("");
    if (!username || !password) {
      setLocalError("Please fill in all fields");
      return;
    }
    if (mode === "register" && password !== confirmPassword) {
      setLocalError("Passwords do not match");
      return;
    }
    await onSubmit(username, password, instance);
  }

  return (
    <div className="p-4">
      <div className="mb-4">
        <h2 className="text-lg font-semibold text-white mb-1">
          {mode === "login" ? "Login to your account" : "Create a new account"}
        </h2>
        <p className="text-xs text-slate-400">
          {mode === "login"
            ? "Enter your credentials to sync your workspace"
            : "Sign up to sync your data across devices"}
        </p>
      </div>

      {displayError && (
        <div className="mb-4 p-3 bg-red-500/10 border border-red-500/20 rounded-lg">
          <p className="text-xs text-red-400">{displayError}</p>
        </div>
      )}

      <div className="space-y-3">
        <div>
          <label className="block text-xs font-medium text-slate-300 mb-1">Username</label>
          <input
            type="text"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
            className={INPUT_CLASS}
            placeholder="johndoe"
            disabled={loading}
          />
        </div>

        <div>
          <label className="block text-xs font-medium text-slate-300 mb-1">Password</label>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
            className={INPUT_CLASS}
            placeholder="••••••••"
            disabled={loading}
          />
        </div>

        {mode === "register" && (
          <div>
            <label className="block text-xs font-medium text-slate-300 mb-1">Confirm Password</label>
            <input
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
              className={INPUT_CLASS}
              placeholder="••••••••"
              disabled={loading}
            />
          </div>
        )}

        <div className="pt-2">
          <button
            onClick={() => setShowAdvanced(!showAdvanced)}
            className="flex items-center gap-2 text-xs text-slate-400 hover:text-slate-300 transition-colors"
          >
            <Icon
              name="chevronRight"
              className={`w-3 h-3 transition-transform ${showAdvanced ? "rotate-90" : ""}`}
            />
            Advanced settings
          </button>

          {showAdvanced && (
            <div className="mt-3">
              <label className="block text-xs font-medium text-slate-300 mb-1">Server Address</label>
              <input
                type="text"
                value={instance}
                onChange={(e) => setInstance(e.target.value)}
                className={INPUT_CLASS}
                placeholder="http://localhost:3000"
                disabled={loading}
              />
            </div>
          )}
        </div>

        <button
          onClick={handleSubmit}
          disabled={loading}
          className="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-600/50 disabled:cursor-not-allowed text-white text-sm font-medium rounded-lg transition-colors"
        >
          {loading
            ? mode === "login" ? "Logging in..." : "Creating account..."
            : mode === "login" ? "Login" : "Create Account"}
        </button>
      </div>

      <div className="mt-4 pt-4 border-t border-slate-700">
        <button
          onClick={onToggleMode}
          className="w-full text-xs text-slate-400 hover:text-slate-300 transition-colors"
        >
          {mode === "login"
            ? "Don't have an account? Create one"
            : "Already have an account? Login"}
        </button>
      </div>
    </div>
  );
}

import { Workspace } from "../../types";
import Icon from "../icons/Icon";

type Props = {
  workspaces: Workspace[];
  current: Workspace | null;
  onSwitch: (name: string) => void;
  onAdd: () => void;
  onLogoutWorkspace: () => void;
};

export default function WorkspaceMenu({ workspaces, current, onSwitch, onAdd, onLogoutWorkspace }: Props) {
  const others = workspaces.filter((w) => w.workspace_name !== current?.workspace_name);

  return (
    <div className="fixed bottom-16 left-0 right-0 mx-4 mb-1 bg-slate-800 border-2 border-slate-700 rounded-lg shadow-xl overflow-y-scroll max-h-96 z-50">
      {others.length > 0 && (
        <div className="py-1">
          <div className="px-2 md:px-3 py-2 text-xs font-medium text-slate-400 uppercase">
            Switch Account
          </div>
          {others.map((w) => (
            <button
              key={w.id}
              onClick={() => onSwitch(w.workspace_name)}
              className="w-full px-2 md:px-3 py-2 flex items-center gap-2 md:gap-3 hover:bg-slate-700 transition-colors text-left"
            >
              <div className="w-7 h-7 md:w-8 md:h-8 rounded-full bg-slate-600 flex items-center justify-center text-white text-xs md:text-sm font-medium">
                {w.workspace_name.charAt(0).toUpperCase()}
              </div>
              <span className="text-xs md:text-sm text-white truncate">{w.workspace_name}</span>
            </button>
          ))}
        </div>
      )}

      <div className="py-1">
        <button
          onClick={onAdd}
          className="w-full px-2 md:px-3 py-2 flex items-center gap-2 md:gap-3 hover:bg-slate-700 transition-colors text-left"
        >
          <span className="text-xs md:text-sm text-white truncate">Add another workspace</span>
        </button>
      </div>

      <div className="border-t border-slate-700 py-1">
        <button
          onClick={onLogoutWorkspace}
          className="w-full px-2 md:px-3 py-2 text-xs md:text-sm text-red-400 hover:bg-slate-700 transition-colors text-left flex items-center gap-2"
        >
          <Icon name="logout" />
          Logout from workspace
        </button>
      </div>
    </div>
  );
}

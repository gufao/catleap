import { SidebarSteam } from "./SidebarSteam";

export type LibraryFilter = "all" | "compatible" | "recent";
export type SourceFilter = "all" | "steam" | "steam_wine" | "manual";

interface SidebarProps {
  libraryFilter: LibraryFilter;
  sourceFilter: SourceFilter;
  onLibraryFilterChange: (filter: LibraryFilter) => void;
  onSourceFilterChange: (filter: SourceFilter) => void;
  onNavigateSettings: () => void;
}

interface NavItemProps {
  label: string;
  active: boolean;
  onClick: () => void;
}

function NavItem({ label, active, onClick }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={`w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-all duration-150 ${
        active
          ? "bg-white shadow-sm text-gray-900"
          : "text-gray-500 hover:text-gray-800 hover:bg-white/60"
      }`}
    >
      {label}
    </button>
  );
}

export function Sidebar({
  libraryFilter,
  sourceFilter,
  onLibraryFilterChange,
  onSourceFilterChange,
  onNavigateSettings,
}: SidebarProps) {
  return (
    <aside className="w-56 h-full bg-gray-50 border-r border-gray-200 flex flex-col py-4 px-3 shrink-0">
      <SidebarSteam />

      <div className="mb-1 px-3 py-1">
        <span className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
          Library
        </span>
      </div>
      <nav className="flex flex-col gap-0.5 mb-5">
        <NavItem
          label="All Games"
          active={libraryFilter === "all"}
          onClick={() => onLibraryFilterChange("all")}
        />
        <NavItem
          label="Compatible"
          active={libraryFilter === "compatible"}
          onClick={() => onLibraryFilterChange("compatible")}
        />
        <NavItem
          label="Recently Played"
          active={libraryFilter === "recent"}
          onClick={() => onLibraryFilterChange("recent")}
        />
      </nav>

      <div className="mb-1 px-3 py-1">
        <span className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
          Sources
        </span>
      </div>
      <nav className="flex flex-col gap-0.5">
        <NavItem
          label="All"
          active={sourceFilter === "all"}
          onClick={() => onSourceFilterChange("all")}
        />
        <NavItem
          label="Steam"
          active={sourceFilter === "steam"}
          onClick={() => onSourceFilterChange("steam")}
        />
        <NavItem
          label="Steam (Windows)"
          active={sourceFilter === "steam_wine"}
          onClick={() => onSourceFilterChange("steam_wine")}
        />
        <NavItem
          label="Manual"
          active={sourceFilter === "manual"}
          onClick={() => onSourceFilterChange("manual")}
        />
      </nav>

      <div className="mt-auto">
        <button
          onClick={onNavigateSettings}
          className="w-full text-left px-3 py-1.5 rounded-md text-sm font-medium text-gray-500 hover:text-gray-800 hover:bg-white/60 transition-all duration-150"
        >
          Settings
        </button>
      </div>
    </aside>
  );
}

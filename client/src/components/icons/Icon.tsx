const PATHS = {
  folder:       "M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z",
  document:     "M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z",
  plus:         "M12 4v16m8-8H4",
  trash:        "M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16",
  restore:      "M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6",
  chevronRight: "M9 5l7 7-7 7",
  chevronUp:    "M5 15l7-7 7 7",
  chevronDown:  "M19 9l-7 7-7-7",
  close:        "M6 18L18 6M6 6l12 12",
  menu:         "M4 6h16M4 12h16M4 18h16",
  warning:      "M12 9v2m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z",
  lock:         "M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z",
  wifi:         "M8.111 16.404a5.5 5.5 0 017.778 0M12 20h.01m-7.08-7.071c3.904-3.905 10.236-3.905 14.141 0M1.394 9.393c5.857-5.857 15.355-5.857 21.213 0",
  search:       "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z",
  user:         "M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z",
  logout:       "M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1",
  logoutIn:     "M7 16l-4-4m0 0l4-4m-4 4h14M11 20v1a3 3 0 003 3h4a3 3 0 003-3V7a3 3 0 00-3-3h-4a3 3 0 00-3 3v1",
  list:         "M8 7h12M8 12h12M8 17h12M3 7h.01M3 12h.01M3 17h.01",
} as const;

type IconName = keyof typeof PATHS;

type Props = {
  name: IconName;
  className?: string;
  strokeWidth?: number;
};

export default function Icon({ name, className = "w-4 h-4", strokeWidth = 2 }: Props) {
  return (
    <svg className={className} fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={strokeWidth} d={PATHS[name]} />
    </svg>
  );
}

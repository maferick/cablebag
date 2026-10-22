// Every keyboard shortcut of Cablebag in words, for Settings, Keyboard shortcuts. The keys themselves are
// handled in App.svelte (window and tiling) and TerminalView.svelte (terminal); keep this list in step.

export interface Shortcut {
  keys: string;
  what: string;
}

export const SHORTCUTS: { group: string; items: Shortcut[] }[] = [
  {
    group: "Finding and opening",
    items: [
      { keys: "Ctrl+K", what: "Search box: sessions, tabs, Proxmox machines, tools, commands (inside a terminal: Ctrl+Shift+K)" },
      { keys: "Ctrl+Shift+N", what: "New session" },
      { keys: "Ctrl+Shift+T", what: "New local terminal" },
      { keys: "Ctrl+Shift+H", what: "Home" },
      { keys: "Ctrl+comma", what: "Settings" },
    ],
  },
  {
    group: "Tabs",
    items: [
      { keys: "Ctrl+Tab", what: "Next tab" },
      { keys: "Ctrl+Shift+Tab", what: "Previous tab" },
      { keys: "Ctrl+Shift+D", what: "Open the same session again in a new tab" },
      { keys: "Ctrl+Shift+W", what: "Close the tab" },
      { keys: "R", what: "Connect again, in a terminal whose connection ended" },
    ],
  },
  {
    group: "Tiling",
    items: [
      { keys: "Ctrl+Alt+arrow", what: "Go to the pane in that direction" },
      { keys: "Ctrl+Alt+Shift+arrow", what: "Move the pane in that direction" },
      { keys: "Ctrl+Alt+Enter", what: "Let the pane fill the area, or back to the tiles" },
      { keys: "Ctrl+Alt+plus, Ctrl+Alt+minus", what: "Make the pane bigger or smaller" },
      { keys: "Ctrl+Alt+0", what: "All panes the same size" },
      { keys: "Ctrl+Alt+R", what: "Turn the split (side by side or above each other)" },
      { keys: "Ctrl+Alt+M", what: "Make it the main pane" },
    ],
  },
  {
    group: "Terminal",
    items: [
      { keys: "Ctrl+Shift+C, Ctrl+Shift+V", what: "Copy, paste" },
      { keys: "Ctrl+Shift+F", what: "Find in the terminal" },
      { keys: "Ctrl+plus, Ctrl+minus", what: "Terminal text bigger or smaller" },
    ],
  },
  {
    group: "Window",
    items: [
      { keys: "Ctrl+Shift+A", what: "Show or hide the assistant" },
      { keys: "Ctrl+Shift+E", what: "Show or hide the remote files of the SSH tab" },
      { keys: "Ctrl+Shift+plus, Ctrl+Shift+minus", what: "Everything in the window bigger or smaller" },
    ],
  },
];

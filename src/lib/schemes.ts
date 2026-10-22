import type { ITheme } from "@xterm/xterm";

export interface Scheme {
  id: string;
  name: string;
  theme: ITheme;
}

export const SCHEMES: Scheme[] = [
  {
    id: "graphite",
    name: "Graphite (dark)",
    theme: {
      background: "#1a1917",
      foreground: "#e4e0d6",
      cursor: "#d8733f",
      cursorAccent: "#1a1917",
      selectionBackground: "#4a3a2e",
      black: "#2a2825", red: "#d9574b", green: "#6aad60", yellow: "#d6a42e",
      blue: "#6f9bd1", magenta: "#c48db7", cyan: "#5fb0a6", white: "#cfc9bc",
      brightBlack: "#7f7a6f", brightRed: "#ef7d70", brightGreen: "#8fc985", brightYellow: "#eec25f",
      brightBlue: "#95bbe6", brightMagenta: "#dcaacf", brightCyan: "#86cdc3", brightWhite: "#f3efe6",
    },
  },
  {
    id: "paper",
    name: "Paper (light)",
    theme: {
      background: "#faf9f5",
      foreground: "#26231f",
      cursor: "#b4461c",
      cursorAccent: "#faf9f5",
      selectionBackground: "#f0d6c4",
      black: "#26231f", red: "#b8271b", green: "#2f7f47", yellow: "#8a5f00",
      blue: "#2b5d93", magenta: "#86397a", cyan: "#1e7770", white: "#d1ccc1",
      brightBlack: "#5f5a51", brightRed: "#cf3b2b", brightGreen: "#2b8248", brightYellow: "#9c6a00",
      brightBlue: "#3570ad", brightMagenta: "#9d4b90", brightCyan: "#1f7f78", brightWhite: "#ffffff",
    },
  },
  {
    id: "classic",
    name: "Classic grey",
    theme: {
      background: "#1e1e1e",
      foreground: "#ececec",
      cursor: "#b4b4c0",
      selectionBackground: "#4a4a55",
      black: "#000000", red: "#ff6060", green: "#60ff60", yellow: "#ffff36",
      blue: "#6c6cff", magenta: "#ff36ff", cyan: "#36ffff", white: "#ececec",
      brightBlack: "#808080", brightRed: "#ff8080", brightGreen: "#80ff80", brightYellow: "#ffff80",
      brightBlue: "#9090ff", brightMagenta: "#ff80ff", brightCyan: "#80ffff", brightWhite: "#ffffff",
    },
  },
  {
    id: "solarized-dark",
    name: "Solarized dark",
    theme: {
      background: "#002b36", foreground: "#839496", cursor: "#93a1a1", selectionBackground: "#073642",
      black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900",
      blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#eee8d5",
      brightBlack: "#586e75", brightRed: "#cb4b16", brightGreen: "#93a1a1", brightYellow: "#839496",
      brightBlue: "#657b83", brightMagenta: "#6c71c4", brightCyan: "#93a1a1", brightWhite: "#fdf6e3",
    },
  },
  {
    id: "solarized-light",
    name: "Solarized light",
    theme: {
      background: "#fdf6e3", foreground: "#657b83", cursor: "#586e75", selectionBackground: "#eee8d5",
      black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900",
      blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#eee8d5",
      brightBlack: "#002b36", brightRed: "#cb4b16", brightGreen: "#586e75", brightYellow: "#657b83",
      brightBlue: "#839496", brightMagenta: "#6c71c4", brightCyan: "#93a1a1", brightWhite: "#fdf6e3",
    },
  },
  {
    id: "monokai",
    name: "Monokai",
    theme: {
      background: "#272822", foreground: "#f8f8f2", cursor: "#f8f8f0", selectionBackground: "#49483e",
      black: "#272822", red: "#f92672", green: "#a6e22e", yellow: "#f4bf75",
      blue: "#66d9ef", magenta: "#ae81ff", cyan: "#a1efe4", white: "#f8f8f2",
      brightBlack: "#75715e", brightRed: "#f92672", brightGreen: "#a6e22e", brightYellow: "#f4bf75",
      brightBlue: "#66d9ef", brightMagenta: "#ae81ff", brightCyan: "#a1efe4", brightWhite: "#f9f8f5",
    },
  },
];

export const schemeById = (id: string) => SCHEMES.find((s) => s.id === id) ?? SCHEMES[0];

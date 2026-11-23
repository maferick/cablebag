// The colours a tab (and its session) can have: muted, so a coloured tab marks a server without shouting.
export const TAB_COLORS = [
  { name: "No colour", value: "" },
  { name: "Brick", value: "#bd4a3e" },
  { name: "Ochre", value: "#ad791c" },
  { name: "Moss", value: "#748427" },
  { name: "Pine", value: "#3f8a55" },
  { name: "Steel", value: "#4f7fa6" },
  { name: "Plum", value: "#8f5e9d" },
  { name: "Stone", value: "#7b7a74" },
];

/** Brick is nearly the rust Cablebag marks the focused pane with, so a profile in it looks focused everywhere. */
const LIKE_THE_ACCENT = "#bd4a3e";

/** The colours a browser profile can have: the tab colours without the one that looks like focus. */
export const PROFILE_COLORS = TAB_COLORS.filter((c) => c.value !== LIKE_THE_ACCENT);

/** Whether a colour is one a profile should no longer have. */
export const looksFocused = (color: string | undefined) => color?.toLowerCase() === LIKE_THE_ACCENT;

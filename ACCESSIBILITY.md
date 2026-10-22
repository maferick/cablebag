# Accessibility

Cablebag is a remote access app for Windows: terminals, Remote Desktop, VNC, files and tools in one window.
This page says what works today for people who use the keyboard, a larger interface or a screen reader,
what does not work yet, and how to tell me about a barrier.

## Our commitment

Cablebag is a small project looked after in spare time, and I want it to be usable without a mouse and
without perfect eyesight. The aim is WCAG 2.2 level AA for the parts of the window that Cablebag draws
itself; that is an aim, not a claim, because no formal test has been done yet. Barriers reported through
the channels below are treated as bugs.

## What works today

Everything here was checked in the source of the current version.

### Keyboard

- Every shortcut is listed in Settings, Keyboard shortcuts (Ctrl+comma opens Settings). The main ones:
  - Ctrl+K opens the search box for sessions, open tabs, Proxmox machines, tools and commands. Inside a
    terminal or remote screen it is Ctrl+Shift+K, because Ctrl+K belongs to the shell there. In the
    search box the arrow keys, Page Up and Page Down move through the results, Enter opens the chosen
    one, Shift+Enter edits a session, Escape closes the box.
  - Tabs: Ctrl+Tab and Ctrl+Shift+Tab switch tabs, Ctrl+Shift+T opens a local terminal, Ctrl+Shift+N a
    new session, Ctrl+Shift+D opens the same session again, Ctrl+Shift+W closes the tab, Ctrl+Shift+H
    shows Home. The tabs in the tab bar and the sessions in the sidebar tree can be reached with Tab and
    opened with Enter.
  - Tiling: Ctrl+Alt+arrow moves between panes, Ctrl+Alt+Shift+arrow moves a pane, Ctrl+Alt+Enter lets
    a pane fill the area and back, Ctrl+Alt+plus and minus change its size, Ctrl+Alt+0 makes all panes
    the same size, Ctrl+Alt+R turns the split, Ctrl+Alt+M makes a pane the main pane. The same actions
    are in the search box and in the menu behind the three-dot button of each pane header, so nothing
    depends on dragging.
  - Terminal: Ctrl+Shift+C and Ctrl+Shift+V copy and paste, Ctrl+Shift+F opens Find (Enter and
    Shift+Enter step through the matches, Escape closes it), Ctrl+plus and minus change the terminal
    text size. When a connection has ended, R connects again.
  - Window: Ctrl+Shift+A shows or hides the assistant, Ctrl+Shift+E the remote files of an SSH tab,
    Ctrl+Shift+plus and minus make everything in the window bigger or smaller.
- Macros can be given their own shortcut; they are listed on the same Keyboard shortcuts page.
- Escape closes the search box, the context menu, the terminal Find bar, the rename box of a tab, and
  the question and confirmation dialogs. Those dialogs put the focus on their text field or OK button
  when they open.
- Context menus are built from ordinary buttons, so Tab reaches their entries.

### Visual

- Interface size: Settings, General, Interface size offers 90, 100, 110, 125, 150 and 175 percent, and
  Ctrl+Shift+plus and minus step through the same list. The whole window scales, including menus and
  dialogs.
- Theme: Settings, General, Theme is Same as Windows (the default), Light or Dark. "Same as Windows"
  follows the Windows light or dark setting and changes with it while Cablebag runs.
- Terminal text: the font and the size (7 to 40) are settings on the Terminal page; the size also has
  Ctrl+plus and minus. The terminal colour scheme is a separate choice (Graphite, Paper, Classic grey,
  Solarized dark, Solarized light, Monokai), so a light interface can keep a dark terminal or the other
  way round.
- The terminal cursor can be a block, an underline or a line, and blinking can be switched off.
- Connection state is not shown by colour alone: connected is a filled circle, connecting a hollow
  ring, failed a diamond, closed a grey circle. The search box also writes the state as a word next to
  each open tab, and the sidebar's reachability dot has a tooltip that describes it.
- The pulsing of the "connecting" ring is switched off when Windows asks for reduced motion.

### Screen readers and assistive technology

- The window is a WebView2 (Chromium) page, so the controls Cablebag draws itself are ordinary HTML
  buttons, inputs, selects and checkboxes, and screen readers can reach them.
- Every icon-only button has a title text, which is also what a screen reader announces.
- Roles are set where the structure is not plain HTML: the tab bar (tablist and tab, with the active
  tab marked selected), the session tree (tree and treeitem, with open and closed folders marked), the
  search results (listbox and option), the context menu (menu and menuitem), dialogs (dialog, modal,
  named after their title), the sidebar and pane splitters (separator), and messages at the bottom of
  the window (status, so they are read out as they appear).
- The search box and the settings filter have labels for screen readers.

## Known limitations

- A Remote Desktop or VNC tab shows a picture of the other computer's screen. Remote Desktop is
  Microsoft's own RDP control in a native child window; VNC is drawn on a canvas. A screen reader on
  this PC cannot read what is inside either of them. Narrator or another screen reader has to run
  inside the remote session, and the keyboard goes to the remote computer while such a tab has the
  focus.
- The terminal is xterm.js, which draws text on a canvas. Its screen reader mode is not switched on and
  there is no setting for it yet, so a screen reader cannot read the terminal output today. Copying
  text out of the terminal and pasting it elsewhere works.
- Dialogs built on the shared dialog frame (session editor, folder settings, import, Proxmox server,
  macro editor, WireGuard editor, tunnels) do not close on Escape and do not keep the focus inside the
  dialog; Tab can leave them. The question and confirmation dialogs are the exception and do close
  on Escape.
- Context menus do not move the focus into themselves when they open and do not react to the arrow
  keys; only Tab, Enter and Escape. Menus on tabs in the tab bar and on sessions in the sidebar open with
  a right-click only; there is no keyboard way to open them yet. Pane headers have a button for theirs.
- The tab bar and the session tree react to Tab and Enter only, not to the arrow keys that are usual for
  tabs and trees.
- Drag and drop (pane swap, pane and sidebar resize by dragging) has keyboard alternatives for the
  panes but none for the sidebar width.
- The keyboard focus is shown with the browser's default ring; a few places (the search box, the file
  browser, the session tree container, the VNC canvas) hide that ring. Focus visibility has not been
  reviewed as a whole.
- Colour contrast of the themes has not been measured against WCAG.
- The busy spinner in the browser tab and in the file browser keeps turning when Windows asks for
  reduced motion; only the connection ring respects that setting. Cursor blinking is a setting, not
  tied to it.
- The terminal shortcuts use Ctrl+Shift with a letter and the tiling shortcuts use Ctrl+Alt, which
  cannot be changed. On keyboards with AltGr the right Alt key is left alone so that it keeps typing
  characters.
- There is no high contrast theme; Windows high contrast mode has not been tested.

## Supported environments

- Windows 10 and Windows 11, 64-bit.
- The window is rendered by the WebView2 runtime that ships with Windows (Edge Chromium). Cablebag
  does not bundle its own browser.
- Assistive technology: no screen reader, magnifier or switch access has been formally tested yet. The
  notes above come from reading the source, not from a test with Narrator, NVDA or JAWS. Reports from
  people who use them are the most useful thing you can send.

## Report a barrier

- Open an issue with the Bug report form at https://github.com/maferick/cablebag/issues/new/choose and
  put the word "accessibility" in the title.
- For a question rather than a bug, use Discussions: https://github.com/maferick/cablebag/discussions

What helps in a report:

- Which assistive technology or Windows setting you use (screen reader and version, magnifier, high
  contrast, reduced motion, keyboard only).
- Where in Cablebag it happens (which tab type, dialog or page) and what you expected instead.
- The Cablebag version (Settings, About Cablebag) and whether Windows is 10 or 11.
- A screenshot or a short recording if the problem is visual.

Last reviewed: October 2026.

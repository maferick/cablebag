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
    shows Home. In the tab bar the left and right arrow keys move between tabs (Home and End to the first
    and last), Enter opens the tab. In the session tree the up and down arrows move between rows, right
    opens a folder and left closes it, Enter opens a session.
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
- Dialogs (session editor, folder settings, import, Proxmox server, macro editor, WireGuard editor,
  questions and confirmations, password and host key prompts) put the focus on their first field when
  they open, keep Tab inside the dialog, close with Escape and give the focus back to where it was.
  With a dialog on top of another, Escape closes only the top one.
- Context menus open with the context menu key or Shift+F10 on a focused tab or session, as well as
  with a right-click. The focus goes to the first entry; the arrow keys, Home and End move between
  entries, Enter runs one, Escape or Tab closes the menu and returns the focus.
- Escape also closes the search box, the terminal Find bar and the rename box of a tab.
- The Servers and Web switch at the top of the sidebar and the squares of pinned sites at the left edge
  are buttons: Tab reaches them, Enter or Space uses them, and the context menu key or Shift+F10 opens a
  pinned site's menu. Each square's name says the site, its browser profile and its unread count.
- The sidebar's width can be changed with the keyboard: Tab to the line between the sidebar and the
  tabs, then the left and right arrows (with Shift in larger steps), Home and End for the narrowest and
  widest.

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
- Keyboard focus is shown with a clear ring in the accent colour on buttons, tabs, rows, fields and
  menus; a mouse click does not draw it.
- Connection state is not shown by colour alone: connected is a filled circle, connecting a hollow
  ring, failed a diamond, closed a grey circle. The search box also writes the state as a word next to
  each open tab, the marks on tabs and pane headers carry the state as a word for screen readers and in
  their tooltip, and the sidebar's reachability dot has a tooltip that describes it.
- When Windows is set to show fewer animations, nothing in Cablebag pulses, spins or slides, and the
  terminal cursor does not blink.

### Screen readers and assistive technology

- The window is a WebView2 (Chromium) page, so the controls Cablebag draws itself are ordinary HTML
  buttons, inputs, selects and checkboxes, and screen readers can reach them.
- Every icon-only button has a name for screen readers (aria-label) as well as its tooltip.
- The terminal has a setting "Screen reader support" (Settings, Terminal). With it on, screen readers
  such as Narrator and NVDA can read the terminal's text and announce new output.
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
- The terminal draws its text on a canvas. With "Screen reader support" off (the default, because it
  slows down very fast output) a screen reader cannot read it; switch it on in Settings, Terminal.
- The remote file editor and the VNC screen show no focus ring of their own: the text cursor and the
  remote screen show where the keyboard goes.
- Colour contrast of the themes has not been measured against WCAG.
- The terminal shortcuts use Ctrl+Shift with a letter and the tiling shortcuts use Ctrl+Alt, which
  cannot be changed. On keyboards with AltGr the right Alt key is left alone so that it keeps typing
  characters.
- There is no high contrast theme; Windows high contrast mode has not been tested.
- Pinned sites can only be put in another order by dragging them on the strip.

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

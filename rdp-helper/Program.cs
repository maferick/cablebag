// Cablebag RDP helper: hosts the Windows Remote Desktop control (mstscax) as a child window of the
// Cablebag main window. Cablebag drives it with one JSON command per line on stdin and reads one JSON
// event per line from stdout.

using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Nodes;
using MSTSCLib;

namespace CablebagRdp;

static class Program
{
    [STAThread]
    static void Main(string[] args)
    {
        long parent = 0;
        for (int i = 0; i < args.Length - 1; i++)
            if (args[i] == "--parent") long.TryParse(args[i + 1], out parent);
        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        Application.EnableVisualStyles();
        FocusWatch.Start();
        Application.Run(new HostForm(new IntPtr(parent)));
    }
}

static class Native
{
    public const int GWL_STYLE = -16, SW_HIDE = 0, SW_SHOWNA = 8;
    public const uint SWP_NOSIZE = 0x1, SWP_NOMOVE = 0x2, SWP_NOACTIVATE = 0x10;
    public static readonly IntPtr HWND_TOP = IntPtr.Zero;

    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SetParent(IntPtr child, IntPtr parent);

    [DllImport("user32.dll")]
    public static extern bool IsWindow(IntPtr hwnd);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);

    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);

    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern int MapWindowPoints(IntPtr from, IntPtr to, ref Point pt, int count);

    public delegate bool EnumProc(IntPtr hwnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool EnumChildWindows(IntPtr parent, EnumProc proc, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetClassName(IntPtr hwnd, System.Text.StringBuilder name, int max);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern short VkKeyScan(char ch);

    [DllImport("user32.dll")]
    public static extern uint MapVirtualKey(uint code, uint mapType);


    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SetWindowLongPtr(IntPtr hwnd, int index, IntPtr value);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hwnd, int cmd);

    [DllImport("user32.dll")]
    public static extern IntPtr GetParent(IntPtr hwnd);

    [DllImport("user32.dll")]
    public static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);

    [DllImport("user32.dll")]
    public static extern IntPtr GetWindowLongPtr(IntPtr hwnd, int index);
}

/// Tells Cablebag when you click the remote screen. The click goes to this process's windows, not to
/// Cablebag, which would otherwise not know which pane you are working in. The remote screen's input
/// window runs on a thread of the Remote Desktop control, so a message filter on our own thread misses
/// it; a focus event from Windows covers every thread of this process.
static class FocusWatch
{
    const uint EVENT_OBJECT_FOCUS = 0x8005, WINEVENT_OUTOFCONTEXT = 0;
    delegate void WinEventProc(IntPtr hook, uint evt, IntPtr hwnd, int obj, int child, uint thread, uint time);

    [DllImport("user32.dll")]
    static extern IntPtr SetWinEventHook(uint min, uint max, IntPtr module, WinEventProc proc, uint pid, uint thread, uint flags);

    [DllImport("user32.dll")]
    static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);

    static WinEventProc? proc;
    static long last;

    /// Must run on a thread with a message loop: Windows delivers the events there.
    public static void Start()
    {
        var me = (uint)Environment.ProcessId;
        proc = (_, _, hwnd, _, _, _, _) =>
        {
            // Every focus change on the desktop arrives here; only one into a window of ours counts.
            GetWindowThreadProcessId(hwnd, out var pid);
            if (pid != me) return;
            var now = Environment.TickCount64;
            if (now - last < 300) return;
            last = now;
            Out.Send("activated");
        };
        SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, IntPtr.Zero, proc, 0, 0, WINEVENT_OUTOFCONTEXT);
    }
}

static class Out
{
    static readonly object Lock = new();

    public static void Send(string evt, object? data = null)
    {
        var node = data is null ? new JsonObject() : JsonSerializer.SerializeToNode(data)!.AsObject();
        node["event"] = evt;
        lock (Lock)
        {
            Console.Out.WriteLine(node.ToJsonString());
            Console.Out.Flush();
        }
    }
}

/// <summary>AxHost for the newest Remote Desktop control registered on this PC.</summary>
class RdpAx : AxHost
{
    // MsTscAx.MsTscAx.13 down to .10, newest first.
    public static readonly string[] Clsids =
    {
        "3F859AA3-C2D4-4FAA-B0E4-FD0C9C4E5E3A",
        "1DF7C823-B2D4-4B54-975A-F2AC5D7CF8B8",
        "A0C63C30-F08D-4AB4-907C-34905D770C7D",
        "8B918B82-7985-4C24-89DF-C33AD2BBFBCD",
    };

    ConnectionPointCookie? cookie;
    readonly IMsTscAxEvents sink;

    public RdpAx(string clsid, IMsTscAxEvents sink) : base(clsid)
    {
        this.sink = sink;
    }

    public object? Ocx => GetOcx();

    protected override void CreateSink()
    {
        try { cookie = new ConnectionPointCookie(GetOcx(), sink, typeof(IMsTscAxEvents)); }
        catch (Exception e) { Out.Send("log", new { text = "event hookup failed: " + e.Message }); }
    }

    protected override void DetachSink()
    {
        try { cookie?.Disconnect(); } catch { }
    }
}

class HostForm : Form, IMsTscAxEvents
{
    const int WS_CHILD = 0x40000000, WS_CLIPCHILDREN = 0x02000000, WS_CLIPSIBLINGS = 0x04000000;

    readonly IntPtr parentHwnd;
    RdpAx? ax;
    IMsRdpClient9? rdp;
    bool connected;
    // Set when the Disconnect button ended the session, so its end is not reported as an error.
    bool userEnded;
    int lastW, lastH;
    readonly System.Windows.Forms.Timer resizeTimer = new() { Interval = 400 };
    JsonObject? pending;

    public HostForm(IntPtr parent)
    {
        parentHwnd = parent;
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        BackColor = Color.FromArgb(29, 28, 26);
        Bounds = new Rectangle(0, 0, 10, 10);
        resizeTimer.Tick += (_, _) => { resizeTimer.Stop(); ApplyRemoteSize(); };
    }

    protected override CreateParams CreateParams
    {
        get
        {
            var cp = base.CreateParams;
            if (parentHwnd != IntPtr.Zero)
            {
                cp.Style = WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS;
                cp.Parent = parentHwnd;
            }
            return cp;
        }
    }

    protected override void SetVisibleCore(bool value)
    {
        // Stay hidden until Cablebag asks to show the window.
        base.SetVisibleCore(IsHandleCreated ? value : false);
        if (!IsHandleCreated) { CreateHandle(); StartReader(); WatchParent(); Out.Send("ready", new { hwnd = Handle.ToInt64() }); }
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        if (parentHwnd == IntPtr.Zero) return;
        // WinForms does not honour a parent window from another process for a Form, so attach it by hand.
        Native.SetWindowLongPtr(Handle, Native.GWL_STYLE,
            new IntPtr(WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS));
        EnsureParent();
    }

    /// <summary>
    /// WinForms moves a child form without a WinForms parent onto its hidden parking window, so put it
    /// back under the Cablebag window whenever that happened.
    /// </summary>
    void EnsureParent()
    {
        if (parentHwnd == IntPtr.Zero || !IsHandleCreated) return;
        if (Native.GetAncestor(Handle, 1) != parentHwnd) Native.SetParent(Handle, parentHwnd);
    }

    /// Quits when the Cablebag window is gone, also when the end of the command pipe never arrives (an
    /// installer that inherited it, a crash): a helper left behind keeps its file locked for updates.
    void WatchParent()
    {
        if (parentHwnd == IntPtr.Zero) return;
        var t = new System.Windows.Forms.Timer { Interval = 1500 };
        t.Tick += (_, _) =>
        {
            if (Native.IsWindow(parentHwnd)) return;
            t.Stop();
            try { Disconnect(); } catch { }
            Environment.Exit(0);
        };
        t.Start();
    }

    void StartReader()
    {
        var t = new Thread(() =>
        {
            string? line;
            while ((line = Console.In.ReadLine()) != null)
            {
                try
                {
                    var cmd = JsonNode.Parse(line)!.AsObject();
                    BeginInvoke(() => Handle_(cmd));
                }
                catch (Exception e) { Out.Send("log", new { text = "bad command: " + e.Message }); }
            }
            // Cablebag went away: close as well.
            try { BeginInvoke(() => { Disconnect(); Application.Exit(); }); } catch { }
        }) { IsBackground = true };
        t.Start();
    }

    static string S(JsonObject o, string k, string d = "") => o[k]?.GetValue<string>() ?? d;
    static int I(JsonObject o, string k, int d = 0) => o[k] is JsonNode n ? (int)n.GetValue<double>() : d;
    static bool B(JsonObject o, string k, bool d = false) => o[k] is JsonNode n ? n.GetValue<bool>() : d;

    void Handle_(JsonObject cmd)
    {
        try
        {
            switch (S(cmd, "cmd"))
            {
                case "bounds":
                    // Coordinates are physical pixels in the Cablebag window's client area.
                    EnsureParent();
                    Native.SetWindowPos(Handle, Native.HWND_TOP, I(cmd, "x"), I(cmd, "y"),
                        Math.Max(1, I(cmd, "w")), Math.Max(1, I(cmd, "h")), Native.SWP_NOACTIVATE);
                    if (connected) { resizeTimer.Stop(); resizeTimer.Start(); }
                    break;
                case "visible":
                    if (B(cmd, "on"))
                    {
                        // WinForms must see the form as visible, or the control never activates its drawing.
                        if (!Visible) Visible = true;
                        EnsureParent();
                        Native.ShowWindow(Handle, Native.SW_SHOWNA);
                        // Keep the remote screen above the web view that fills the rest of the window.
                        Native.SetWindowPos(Handle, Native.HWND_TOP, 0, 0, 0, 0,
                            Native.SWP_NOMOVE | Native.SWP_NOSIZE | Native.SWP_NOACTIVATE);                    }
                    else
                    {
                        Native.ShowWindow(Handle, Native.SW_HIDE);
                    }
                    break;
                case "connect":
                    Connect(cmd);
                    break;
                case "focus":
                    ax?.Focus();
                    break;
                case "cad":
                    SendCtrlAltDel();
                    break;
                case "snapshot":
                    Snapshot(S(cmd, "req"));
                    break;
                case "mouse":
                    Mouse(cmd);
                    Out.Send("reply", new { req = S(cmd, "req") });
                    break;
                case "keys":
                    PressKeys(S(cmd, "keys"));
                    Out.Send("reply", new { req = S(cmd, "req") });
                    break;
                case "type":
                    Out.Send("reply", new { req = S(cmd, "req"), skipped = TypeText(S(cmd, "text")) });
                    break;
                case "disconnect":
                    userEnded = true;
                    Disconnect();
                    break;
                case "quit":
                    Disconnect();
                    Application.Exit();
                    break;
            }
        }
        catch (Exception e)
        {
            // A request from the assistant gets its error back; anything else shows in the tab.
            if (cmd["req"] is not null) Out.Send("reply", new { req = S(cmd, "req"), error = e.Message });
            else Out.Send("error", new { text = e.Message });
        }
    }

    void Connect(JsonObject c)
    {
        if (ax == null)
        {
            Exception? last = null;
            foreach (var id in RdpAx.Clsids)
            {
                try
                {
                    var a = new RdpAx(id, this) { Dock = DockStyle.Fill };
                    ((System.ComponentModel.ISupportInitialize)a).BeginInit();
                    Controls.Add(a);
                    ((System.ComponentModel.ISupportInitialize)a).EndInit();
                    _ = a.Handle; // creates and activates the ActiveX control even while hidden
                    rdp = (IMsRdpClient9)(a.Ocx ?? throw new Exception("control did not start"));
                    ax = a;
                    break;
                }
                catch (Exception e)
                {
                    last = e;
                    foreach (Control ctl in Controls) ctl.Dispose();
                    Controls.Clear();
                }
            }
            if (rdp == null) throw new Exception("The Windows Remote Desktop control could not be loaded: " + last?.Message);
        }
        var r = rdp!;
        var adv = r.AdvancedSettings9;
        var o = c["options"] as JsonObject ?? new JsonObject();

        r.Server = S(c, "host");
        adv.RDPPort = I(c, "port", 3389);
        var user = S(c, "user");
        var domain = S(c, "domain");
        if (user.Contains('\\') && domain == "")
        {
            domain = user[..user.IndexOf('\\')];
            user = user[(user.IndexOf('\\') + 1)..];
        }
        if (user != "") r.UserName = user;
        if (domain != "") r.Domain = domain;
        var pw = S(c, "password");
        if (pw != "") ((IMsTscNonScriptable)r).ClearTextPassword = pw;

        int w = Math.Max(200, I(c, "width", ClientSize.Width));
        int h = Math.Max(200, I(c, "height", ClientSize.Height));
        var fixedRes = S(o, "resolution", "fit");
        if (fixedRes.Contains('x'))
        {
            var p = fixedRes.Split('x');
            w = int.Parse(p[0]);
            h = int.Parse(p[1]);
        }
        r.DesktopWidth = w;
        r.DesktopHeight = h;
        lastW = w;
        lastH = h;
        var depth = I(o, "color_depth", 32);
        r.ColorDepth = depth is 8 or 15 or 16 or 24 or 32 ? depth : 32;

        adv.SmartSizing = B(o, "smart_sizing", fixedRes.Contains('x'));
        adv.EnableCredSspSupport = B(o, "credssp", true);
        adv.AuthenticationLevel = (uint)I(o, "auth_level", 2);
        adv.EnableAutoReconnect = B(o, "auto_reconnect", true);
        adv.Compress = B(o, "compression", true) ? 1 : 0;
        adv.BitmapPersistence = 1;
        adv.RedirectClipboard = B(o, "redirect_clipboard", true);
        adv.RedirectDrives = B(o, "redirect_drives", false);
        adv.RedirectPrinters = B(o, "redirect_printers", false);
        adv.RedirectPorts = B(o, "redirect_ports", false);
        adv.RedirectSmartCards = B(o, "redirect_smartcards", false);
        adv.ConnectToAdministerServer = B(o, "admin_console", false);
        adv.GrabFocusOnConnect = true;
        adv.ContainerHandledFullScreen = 1;
        adv.allowBackgroundInput = 1;
        adv.DisplayConnectionBar = false;

        var sec = (IMsRdpClientSecuredSettings2)r.SecuredSettings3;
        sec.AudioRedirectionMode = S(o, "audio", "local") switch { "remote" => 1, "none" => 2, _ => 0 };
        sec.KeyboardHookMode = B(o, "forward_shortcuts", true) ? 1 : 2;
        var prog = S(o, "remote_program");
        if (prog != "") { sec.StartProgram = prog; }

        try { adv.AudioCaptureRedirectionMode = B(o, "redirect_microphone", false); } catch { }
        try { adv.EnableSuperPan = false; } catch { }
        try
        {
            // Visual experience: Performance flags. Wallpaper, themes and animations off unless asked for.
            int flags = 0;
            if (!B(o, "wallpaper", false)) flags |= 0x1;
            if (!B(o, "full_window_drag", false)) flags |= 0x2;
            if (!B(o, "animations", false)) flags |= 0x4;
            if (!B(o, "themes", true)) flags |= 0x8;
            if (B(o, "font_smoothing", true)) flags |= 0x80;
            if (B(o, "desktop_composition", true)) flags |= 0x100;
            adv.PerformanceFlags = flags;
        }
        catch { }

        var ext = (IMsRdpExtendedSettings)r;
        var scale = I(c, "scale", 100);
        try { ext.set_Property("DesktopScaleFactor", (uint)scale); } catch { }
        try { ext.set_Property("DeviceScaleFactor", (uint)(scale >= 175 ? 180 : scale >= 125 ? 140 : 100)); } catch { }
        if (B(o, "restricted_admin", false)) try { ext.set_Property("RestrictedLogon", true); } catch { }
        if (B(o, "remote_guard", false)) try { ext.set_Property("RedirectionWarningType", 0); ext.set_Property("RemoteCredentialGuard", true); } catch { }

        var gw = o["gateway"] as JsonObject;
        if (gw != null && S(gw, "host") != "")
        {
            var t = r.TransportSettings4;
            t.GatewayHostname = S(gw, "host");
            t.GatewayUsageMethod = S(gw, "usage", "always") == "detect" ? 2u : 1u;
            t.GatewayProfileUsageMethod = 1;
            t.GatewayCredsSource = S(gw, "auth", "same") switch { "smartcard" => 1u, "ask" => 4u, _ => 0u };
            if (S(gw, "auth", "same") == "same") t.GatewayCredSharing = 1;
            if (S(gw, "user") != "") t.GatewayUsername = S(gw, "user");
        }

        pending = null;
        Out.Send("connecting");
        r.Connect();
    }

    void ApplyRemoteSize()
    {
        if (rdp == null || !connected) return;
        int w = ClientSize.Width, h = ClientSize.Height;
        if (w < 200 || h < 200 || (w == lastW && h == lastH)) return;
        if (rdp.AdvancedSettings9.SmartSizing) return;
        try
        {
            ((IMsRdpClient9)rdp).UpdateSessionDisplaySettings((uint)w, (uint)h, (uint)w, (uint)h, 0, 100, 100);
            lastW = w;
            lastH = h;
        }
        catch (Exception e) { Out.Send("log", new { text = "resize: " + e.Message }); }
    }

    void SendCtrlAltDel()
    {
        if (rdp == null || !connected) return;
        // Scan codes: Ctrl 0x1D, Alt 0x38, Delete 0x53. Press all three, then release.
        ax?.Focus();
        SendScanCodes(new() { (0x1D, false), (0x38, false), (0x53, false), (0x53, true), (0x38, true), (0x1D, true) });
    }

    /// A picture of the remote screen for the assistant, as it shows in Cablebag now. Windows renders the whole
    /// Cablebag window (also when other windows cover it, and with the control's DirectX drawing), and only
    /// this pane is cut out of it: a copy of the screen could hold another program's window instead.
    void Snapshot(string req)
    {
        if (!connected || !Visible || parentHwnd == IntPtr.Zero)
        {
            Out.Send("reply", new { req, error = "The remote screen is not showing." });
            return;
        }
        var r = RectangleToScreen(ClientRectangle);
        Native.GetWindowRect(parentHwnd, out var w);
        using var window = new Bitmap(Math.Max(1, w.Right - w.Left), Math.Max(1, w.Bottom - w.Top));
        using (var g = Graphics.FromImage(window))
        {
            var hdc = g.GetHdc();
            const uint PW_RENDERFULLCONTENT = 2;
            var ok = Native.PrintWindow(parentHwnd, hdc, PW_RENDERFULLCONTENT);
            g.ReleaseHdc(hdc);
            if (!ok)
            {
                Out.Send("reply", new { req, error = "Windows did not give a picture of the window." });
                return;
            }
        }
        using var full = window.Clone(Rectangle.Intersect(new Rectangle(r.X - w.Left, r.Y - w.Top, r.Width, r.Height), new Rectangle(Point.Empty, window.Size)), window.PixelFormat);
        // At most 1600 pixels wide or high: enough to read the screen, and the request stays small.
        var scale = Math.Min(1.0, 1600.0 / Math.Max(full.Width, full.Height));
        using var img = new Bitmap(Math.Max(1, (int)(full.Width * scale)), Math.Max(1, (int)(full.Height * scale)));
        using (var g = Graphics.FromImage(img))
        {
            g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBicubic;
            g.DrawImage(full, 0, 0, img.Width, img.Height);
        }
        using var ms = new MemoryStream();
        var jpeg = System.Drawing.Imaging.ImageCodecInfo.GetImageEncoders().First(c => c.FormatID == System.Drawing.Imaging.ImageFormat.Jpeg.Guid);
        using var p = new System.Drawing.Imaging.EncoderParameters(1);
        p.Param[0] = new System.Drawing.Imaging.EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 85L);
        img.Save(ms, jpeg, p);
        Out.Send("reply", new { req, image = Convert.ToBase64String(ms.ToArray()) });
    }

    /// The remote screen's input window: the control turns its mouse messages into input for the remote computer.
    IntPtr InputWindow()
    {
        var found = IntPtr.Zero;
        Native.EnumChildWindows(Handle, (h, _) =>
        {
            var name = new System.Text.StringBuilder(64);
            Native.GetClassName(h, name, 64);
            if (name.ToString() != "IHWindowClass") return true;
            found = h;
            return false;
        }, IntPtr.Zero);
        return found;
    }

    /// A click, double click, move or scroll at a point given from 0 to 1000 across and down the screen.
    void Mouse(JsonObject cmd)
    {
        var ih = InputWindow();
        if (rdp == null || !connected || ih == IntPtr.Zero) throw new Exception("The remote screen is not connected.");
        var pt = new Point(
            Math.Clamp((int)Math.Round(I(cmd, "x") / 1000.0 * ClientSize.Width), 0, ClientSize.Width - 1),
            Math.Clamp((int)Math.Round(I(cmd, "y") / 1000.0 * ClientSize.Height), 0, ClientSize.Height - 1));
        Native.MapWindowPoints(Handle, ih, ref pt, 1);
        var at = new IntPtr((pt.Y << 16) | (pt.X & 0xFFFF));
        var (down, up, held) = S(cmd, "button", "left") switch
        {
            "right" => (0x204u, 0x205u, 0x2),
            "middle" => (0x207u, 0x208u, 0x10),
            _ => (0x201u, 0x202u, 0x1),
        };
        Native.PostMessage(ih, 0x200, IntPtr.Zero, at);
        var action = S(cmd, "action", "click");
        if (action == "scroll")
        {
            // The wheel message takes screen coordinates; one notch is 120, up is positive.
            var sp = new Point(pt.X, pt.Y);
            Native.MapWindowPoints(ih, IntPtr.Zero, ref sp, 1);
            var notches = I(cmd, "amount", -3);
            Native.PostMessage(ih, 0x20A, new IntPtr((notches * 120) << 16), new IntPtr((sp.Y << 16) | (sp.X & 0xFFFF)));
            return;
        }
        var clicks = action switch { "double" => 2, "move" => 0, _ => 1 };
        for (int i = 0; i < clicks; i++)
        {
            Native.PostMessage(ih, down, new IntPtr(held), at);
            Native.PostMessage(ih, up, IntPtr.Zero, at);
        }
    }

    /// Scan codes of named keys; 0x100 marks an extended key.
    static readonly Dictionary<string, int> KeyCodes = new()
    {
        ["ctrl"] = 0x1D, ["control"] = 0x1D, ["shift"] = 0x2A, ["alt"] = 0x38, ["win"] = 0x15B, ["windows"] = 0x15B,
        ["enter"] = 0x1C, ["return"] = 0x1C, ["esc"] = 0x01, ["escape"] = 0x01, ["tab"] = 0x0F, ["space"] = 0x39,
        ["backspace"] = 0x0E, ["delete"] = 0x153, ["del"] = 0x153, ["insert"] = 0x152, ["home"] = 0x147, ["end"] = 0x14F,
        ["pageup"] = 0x149, ["pagedown"] = 0x151, ["up"] = 0x148, ["down"] = 0x150, ["left"] = 0x14B, ["right"] = 0x14D,
        ["f1"] = 0x3B, ["f2"] = 0x3C, ["f3"] = 0x3D, ["f4"] = 0x3E, ["f5"] = 0x3F, ["f6"] = 0x40, ["f7"] = 0x41,
        ["f8"] = 0x42, ["f9"] = 0x43, ["f10"] = 0x44, ["f11"] = 0x57, ["f12"] = 0x58,
    };

    /// The scan code of one character on this PC's keyboard layout and whether it needs Shift; null when no key types it.
    static (int code, bool shift)? CharKey(char ch)
    {
        var vk = Native.VkKeyScan(ch);
        if (vk == -1 || (vk & 0x600) != 0) return null;
        var code = (int)Native.MapVirtualKey((uint)(vk & 0xFF), 0);
        return code == 0 ? null : (code, (vk & 0x100) != 0);
    }

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    delegate int SendKeysFn(IntPtr self, int numKeys, short[] keyUp, int[] keyData);

    /// Presses and releases scan codes through the control, in one call: keys held down (Ctrl, Shift) are let go
    /// at the end of each call. The call goes straight to the control, because its up flags are 2-byte values
    /// and the .NET wrapper passes a bool array with one byte per flag, which only works for a single key.
    void SendScanCodes(List<(int code, bool up)> keys)
    {
        if (rdp == null || !connected) throw new Exception("The remote screen is not connected.");
        if (keys.Count == 0) return;
        var unknown = Marshal.GetIUnknownForObject(rdp);
        var iid = typeof(IMsRdpClientNonScriptable).GUID;
        try
        {
            Marshal.ThrowExceptionForHR(Marshal.QueryInterface(unknown, ref iid, out var self));
            try
            {
                // The wrapper lists every method of the interface in table order, after IUnknown's three.
                var methods = typeof(IMsRdpClientNonScriptable).GetMethods().Select(m => m.Name).ToList();
                var slot = 3 + methods.IndexOf("SendKeys");
                var table = Marshal.ReadIntPtr(self);
                var send = Marshal.GetDelegateForFunctionPointer<SendKeysFn>(Marshal.ReadIntPtr(table, slot * IntPtr.Size));
                var up = keys.Select(k => (short)(k.up ? -1 : 0)).ToArray();
                var codes = keys.Select(k => k.code).ToArray();
                Marshal.ThrowExceptionForHR(send(self, keys.Count, up, codes));
            }
            finally { Marshal.Release(self); }
        }
        finally { Marshal.Release(unknown); }
    }

    /// A key or combination such as "enter", "ctrl+l" or "win+r": the modifiers go down first and up last.
    void PressKeys(string combo)
    {
        var codes = new List<int>();
        foreach (var raw in combo.ToLowerInvariant().Split('+', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
        {
            if (KeyCodes.TryGetValue(raw, out var c)) codes.Add(c);
            else if (raw.Length == 1 && CharKey(raw[0]) is { } k) codes.Add(k.code);
            else throw new Exception($"Unknown key \"{raw}\".");
        }
        var seq = codes.Select(c => (c, false)).Concat(Enumerable.Reverse(codes).Select(c => (c, true))).ToList();
        SendScanCodes(seq);
    }

    /// Types text as key presses. Returns the characters this keyboard layout cannot type.
    string TypeText(string text)
    {
        var skipped = new System.Text.StringBuilder();
        // One call per character, so Shift is held for exactly that character.
        foreach (var ch in text.Replace("\r\n", "\n"))
        {
            if (ch == '\n') { SendScanCodes(new() { (0x1C, false), (0x1C, true) }); continue; }
            if (ch == '\t') { SendScanCodes(new() { (0x0F, false), (0x0F, true) }); continue; }
            if (CharKey(ch) is not { } k) { skipped.Append(ch); continue; }
            var seq = new List<(int, bool)>();
            if (k.shift) seq.Add((0x2A, false));
            seq.Add((k.code, false));
            seq.Add((k.code, true));
            if (k.shift) seq.Add((0x2A, true));
            SendScanCodes(seq);
        }
        return skipped.ToString();
    }

    void Disconnect()
    {
        try { if (rdp != null && rdp.Connected != 0) rdp.Disconnect(); } catch { }
    }

    // ---- control events ----
    public void OnConnecting() => Out.Send("connecting");
    public void OnConnected() { connected = true; Out.Send("connected"); }
    public void OnLoginComplete() { Out.Send("login"); BeginInvoke(ApplyRemoteSize); }
    public void OnDisconnected(int discReason)
    {
        connected = false;
        string text = "";
        // The plain reasons (1 local, 2 ended on the remote computer, 3 by the server) have no real error behind
        // them; the control describes 1 as "An internal error has occurred."
        if (userEnded || discReason == 1) text = "You disconnected the session.";
        else if (discReason == 2) text = "The session was ended on the remote computer.";
        else if (discReason == 3) text = "The remote computer ended the session.";
        else try { text = rdp!.GetErrorDescription((uint)discReason, (uint)rdp.ExtendedDisconnectReason); } catch { }
        userEnded = false;
        Out.Send("disconnected", new { code = discReason, text });
    }
    public void OnEnterFullScreenMode() { }
    public void OnLeaveFullScreenMode() { }
    public void OnChannelReceivedData(string chanName, string data) { }
    public void OnRequestGoFullScreen() => Out.Send("fullscreen", new { on = true });
    public void OnRequestLeaveFullScreen() => Out.Send("fullscreen", new { on = false });
    public void OnFatalError(int errorCode) => Out.Send("fatal", new { code = errorCode });
    public void OnWarning(int warningCode) => Out.Send("warning", new { code = warningCode });
    public void OnRemoteDesktopSizeChange(int width, int height) => Out.Send("size", new { width, height });
    public void OnIdleTimeoutNotification() { }
    public void OnRequestContainerMinimize() { }
    public void OnConfirmClose(out bool pfAllowClose) => pfAllowClose = true;
    public void OnReceivedTSPublicKey(string publicKey, out bool pfContinueLogon) => pfContinueLogon = true;
    public void OnAutoReconnecting(int disconnectReason, int attemptCount, out AutoReconnectContinueState pArcContinueStatus)
    {
        pArcContinueStatus = AutoReconnectContinueState.autoReconnectContinueAutomatic;
        Out.Send("reconnecting", new { attempt = attemptCount });
    }
    public void OnAuthenticationWarningDisplayed() => Out.Send("auth-warning", new { shown = true });
    public void OnAuthenticationWarningDismissed() => Out.Send("auth-warning", new { shown = false });
    public void OnRemoteProgramResult(string bstrRemoteProgram, RemoteProgramResult lError, bool vbIsExecutable) { }
    public void OnRemoteProgramDisplayed(bool vbDisplayed, uint uDisplayInformation) { }
    public void OnRemoteWindowDisplayed(bool vbDisplayed, ref _RemotableHandle hwnd, RemoteWindowDisplayedAttribute windowAttribute) { }
    public void OnLogonError(int lError) => Out.Send("logon-error", new { code = lError });
    public void OnFocusReleased(int iDirection) => Out.Send("focus-released", new { direction = iDirection });
    public void OnUserNameAcquired(string bstrUserName) { }
    public void OnMouseInputModeChanged(bool fMouseModeRelative) { }
    public void OnServiceMessageReceived(string serviceMessage) { }
    public void OnConnectionBarPullDown() { }
    public void OnNetworkStatusChanged(uint qualityLevel, int bandwidth, int rtt) { }
    public void OnDevicesButtonPressed() { }
    public void OnAutoReconnected() { connected = true; Out.Send("connected"); }
    public void OnAutoReconnecting2(int disconnectReason, bool networkAvailable, int attemptCount, int maxAttemptCount) { }
}

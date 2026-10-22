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
        if (!IsHandleCreated) { CreateHandle(); StartReader(); Out.Send("ready", new { hwnd = Handle.ToInt64() }); }
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
                    Out.Send("log", new { text = $"visible={Visible} ax={(ax == null ? "none" : $"{ax.Visible} {ax.Bounds}")} form={ClientSize} parent-ok={Native.GetAncestor(Handle, 1) == parentHwnd}" });
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
                case "disconnect":
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
            Out.Send("error", new { text = e.Message });
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
        var ns = (IMsRdpClientNonScriptable)rdp;
        // Scan codes: Ctrl 0x1D, Alt 0x38, Delete 0x53 (extended). Press all three, then release.
        bool[] keyUp = { false, false, false, true, true, true };
        int[] keys = { 0x1D, 0x38, 0x53, 0x53, 0x38, 0x1D };
        ax?.Focus();
        ns.SendKeys(6, ref keyUp[0], ref keys[0]);
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
        try { text = rdp!.GetErrorDescription((uint)discReason, (uint)rdp.ExtendedDisconnectReason); } catch { }
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

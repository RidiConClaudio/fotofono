using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace Fotofono;

/// <summary>
/// A WebView2 window around docs/index.html (the same page as the website),
/// plus what a browser can't do well: native "Save as", "Open with", offline fonts, remembered window.
/// </summary>
sealed class MainForm : Form
{
    const string Host = "fotofono.local";
    const string Origin = "https://" + Host + "/";

    // FOTOFONO_DATA lets you use another data folder (for example for automated tests)
    public static readonly string DataDir =
        Environment.GetEnvironmentVariable("FOTOFONO_DATA") is { Length: > 0 } custom ? custom :
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Fotofono");
    static readonly string PlacementFile = Path.Combine(DataDir, "window.json");

    readonly WebView2 web = new() { Dock = DockStyle.Fill };
    readonly string? startFile;
    bool startOpened;

    public MainForm(string? startFile)
    {
        this.startFile = startFile;
        Text = "Fotofono";
        Icon = LoadIcon();
        MinimumSize = new Size(480, 560);
        RestorePlacement();

        // The interface is a dark rack: window and title bar share the chassis colour
        BackColor = Color.FromArgb(0x12, 0x14, 0x18);
        web.DefaultBackgroundColor = BackColor;
        Controls.Add(web);

        // Shown during the ~2 s the page engine needs to start
        var splash = new Label
        {
            Text = Strings.Get("splash"),
            Dock = DockStyle.Fill,
            TextAlign = ContentAlignment.MiddleCenter,
            ForeColor = Color.FromArgb(0xFF, 0xB2, 0x3E),
            BackColor = BackColor,
            Font = new Font("Bahnschrift", 13f, FontStyle.Regular),
        };
        Controls.Add(splash);
        splash.BringToFront();
        web.NavigationCompleted += (_, _) => { if (!splash.IsDisposed) { Controls.Remove(splash); splash.Dispose(); } };

        HandleCreated += (_, _) => StyleTitleBar();
        FormClosing += (_, _) => SavePlacement();
        Load += async (_, _) => await InitAsync();
    }

    async Task InitAsync()
    {
        try
        {
            var env = await CoreWebView2Environment.CreateAsync(null, Path.Combine(DataDir, "WebView2"));
            await web.EnsureCoreWebView2Async(env);
        }
        catch (WebView2RuntimeNotFoundException)
        {
            var r = MessageBox.Show(this, Strings.Get("noWebView"), "Fotofono", MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
            if (r == DialogResult.Yes) OpenExternal("https://go.microsoft.com/fwlink/p/?LinkId=2124703");
            Close();
            return;
        }
        catch (Exception ex)
        {
            Program.Log(ex);
            MessageBox.Show(this, Strings.Get("engineFail") + "\n\n" + ex.Message, "Fotofono", MessageBoxButtons.OK, MessageBoxIcon.Error);
            Close();
            return;
        }

        var cw = web.CoreWebView2;
        var s = cw.Settings;
#if DEBUG
        s.AreDevToolsEnabled = true;
#else
        s.AreDevToolsEnabled = false;
#endif
        s.AreDefaultContextMenusEnabled = false;
        s.AreBrowserAcceleratorKeysEnabled = false;
        s.IsStatusBarEnabled = false;
        s.IsGeneralAutofillEnabled = false;
        s.IsPasswordAutosaveEnabled = false;
        // The page is embedded in the executable and external links open in the default browser:
        // checking every navigation with SmartScreen protects nothing here and cost about 2 s at startup
        s.IsReputationCheckingRequired = false;

        // The page files are extracted once to a folder and served from a virtual host:
        // the fastest option (intercepted responses added about 2 s to startup)
        cw.SetVirtualHostNameToFolderMapping(Host, ExtractSite(), CoreWebView2HostResourceAccessKind.Deny);
        cw.DownloadStarting += OnDownloadStarting;
        cw.WebMessageReceived += OnWebMessage;
        cw.DocumentTitleChanged += (_, _) => Text = string.IsNullOrWhiteSpace(cw.DocumentTitle) ? "Fotofono" : cw.DocumentTitle;
        cw.NewWindowRequested += (_, e) => { e.Handled = true; OpenExternal(e.Uri); };
        cw.NavigationStarting += (_, e) =>
        {
            if (e.Uri.StartsWith(Origin, StringComparison.OrdinalIgnoreCase)) return;
            e.Cancel = true;
            OpenExternal(e.Uri);
        };
        cw.NavigationCompleted += (_, e) =>
        {
            if (e.IsSuccess && startFile != null && !startOpened) { startOpened = true; OpenImage(startFile); }
        };
        cw.ProcessFailed += (_, e) =>
        {
            Program.Log(new Exception("WebView2 ProcessFailed: " + e.ProcessFailedKind + " / " + e.Reason));
            if (e.ProcessFailedKind is CoreWebView2ProcessFailedKind.RenderProcessExited or CoreWebView2ProcessFailedKind.RenderProcessUnresponsive)
                BeginInvoke(() => cw.Reload());
        };

        cw.Navigate(Origin + "index.html");
    }

    // ---- messages from the page ----

    void OnWebMessage(object? sender, CoreWebView2WebMessageReceivedEventArgs e)
    {
        try
        {
            using var doc = JsonDocument.Parse(e.WebMessageAsJson);
            var root = doc.RootElement;
            if (root.ValueKind == JsonValueKind.Object && root.TryGetProperty("type", out var t) && t.GetString() == "lang" &&
                root.TryGetProperty("lang", out var l))
                Strings.Set(l.GetString());
        }
        catch (Exception ex) { Program.Log(ex); }
    }

    // ---- the page and its files, extracted from the executable (all embedded: works offline) ----

    static string ExtractSite()
    {
        var page = Encoding.UTF8.GetBytes(BuildPage());
        var tag = Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(page))[..12];
        var root = Path.Combine(DataDir, "app");
        var dir = Path.Combine(root, tag);
        if (!File.Exists(Path.Combine(dir, "index.html")))
        {
            var tmp = dir + ".tmp";
            if (Directory.Exists(tmp)) Directory.Delete(tmp, true);
            Directory.CreateDirectory(Path.Combine(tmp, "fonts"));
            foreach (var name in typeof(MainForm).Assembly.GetManifestResourceNames())
                if (name.StartsWith("fonts/") && name.EndsWith(".woff2"))
                    File.WriteAllBytes(Path.Combine(tmp, "fonts", Path.GetFileName(name)), ReadResource(name)!);
            File.WriteAllBytes(Path.Combine(tmp, "favicon.ico"), ReadResource("fotofono.ico")!);
            File.WriteAllBytes(Path.Combine(tmp, "index.html"), page);
            try { Directory.Move(tmp, dir); }
            catch (IOException) { if (!Directory.Exists(dir)) throw; } // another instance just created it
        }
        // previous versions of the page are no longer needed
        foreach (var old in Directory.GetDirectories(root))
            if (!string.Equals(Path.GetFileName(old), tag, StringComparison.OrdinalIgnoreCase) && !old.EndsWith(".tmp"))
                try { Directory.Delete(old, true); } catch { }
        return dir;
    }

    static string BuildPage()
    {
        // Same page as the website; here the fonts come from the executable instead of Google Fonts
        var page = ReadResourceText("index.html");
        page = Regex.Replace(page, "<link[^>]+fonts\\.(googleapis|gstatic)\\.com[^>]*>\\s*", "");
        var fonts = "<style>" + ReadResourceText("fonts/fonts.css") + "</style>\n";
        var at = page.IndexOf("</head>", StringComparison.OrdinalIgnoreCase);
        return at < 0 ? page : page.Insert(at, fonts);
    }

    void OpenImage(string file)
    {
        var name = Path.GetFileName(file);
        try
        {
            var info = new FileInfo(file);
            if (info.Length > 80L * 1024 * 1024) { Toast("msg.tooBig", name); return; }
            var b64 = Convert.ToBase64String(File.ReadAllBytes(file));
            web.CoreWebView2.PostWebMessageAsJson(JsonSerializer.Serialize(new { type = "open", name, mime = MimeFor(file), b64 }));
        }
        catch (Exception ex) { Program.Log(ex); Toast("msg.cantOpen", name); }
    }

    // ---- saving sounds with the native "Save as" dialog ----

    void OnDownloadStarting(object? sender, CoreWebView2DownloadStartingEventArgs e)
    {
        var deferral = e.GetDeferral();
        var suggested = Path.GetFileName(e.ResultFilePath);
        BeginInvoke(() =>
        {
            try
            {
                var ext = Path.GetExtension(suggested).ToLowerInvariant();
                using var dlg = new SaveFileDialog
                {
                    Title = Strings.Get("saveTitle"),
                    FileName = suggested,
                    Filter = ext == ".zip" ? Strings.Get("zip") + " (*.zip)|*.zip" : Strings.Get("wav") + " (*.wav)|*.wav",
                    InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyMusic),
                    OverwritePrompt = true,
                };
                if (dlg.ShowDialog(this) == DialogResult.OK)
                {
                    e.ResultFilePath = dlg.FileName;
                    e.Handled = true;
                    var op = e.DownloadOperation;
                    var target = dlg.FileName;
                    op.StateChanged += (_, _) =>
                    {
                        if (op.State == CoreWebView2DownloadState.Completed) Toast("msg.saved", Path.GetFileName(target));
                        else if (op.State == CoreWebView2DownloadState.Interrupted) Toast("msg.saveFailed");
                    };
                }
                else
                {
                    e.Cancel = true;
                }
            }
            catch (Exception ex)
            {
                Program.Log(ex);
                e.Cancel = true;
                Toast("msg.saveFailed");
            }
            finally { deferral.Complete(); }
        });
    }

    /// <summary>Shows a message in the page; the page translates the key into the current language.</summary>
    void Toast(string key, string? arg = null) =>
        web.CoreWebView2?.PostWebMessageAsJson(JsonSerializer.Serialize(new { type = "toast", key, arg }));

    // ---- window position and size, remembered between launches ----

    sealed record Placement(int X, int Y, int W, int H, bool Max);

    void RestorePlacement()
    {
        try
        {
            var p = JsonSerializer.Deserialize<Placement>(File.ReadAllText(PlacementFile));
            if (p != null && p.W >= MinimumSize.Width && p.H >= MinimumSize.Height)
            {
                var r = new Rectangle(p.X, p.Y, p.W, p.H);
                // only if the title bar lands on a screen that is connected now
                if (Screen.AllScreens.Any(sc => sc.WorkingArea.IntersectsWith(new Rectangle(r.X + 40, r.Y, Math.Max(1, r.Width - 80), 30))))
                {
                    StartPosition = FormStartPosition.Manual;
                    Bounds = r;
                    if (p.Max) WindowState = FormWindowState.Maximized;
                    return;
                }
            }
        }
        catch { /* first launch or unreadable file: default position */ }

        StartPosition = FormStartPosition.CenterScreen;
        var area = Screen.PrimaryScreen?.WorkingArea ?? new Rectangle(0, 0, 1600, 1000);
        Size = new Size(Math.Min(1400, area.Width - 80), Math.Min(940, area.Height - 60));
    }

    void SavePlacement()
    {
        try
        {
            var b = WindowState == FormWindowState.Normal ? Bounds : RestoreBounds;
            Directory.CreateDirectory(DataDir);
            File.WriteAllText(PlacementFile, JsonSerializer.Serialize(
                new Placement(b.X, b.Y, b.Width, b.Height, WindowState == FormWindowState.Maximized)));
        }
        catch (Exception ex) { Program.Log(ex); }
    }

    // ---- helpers ----

    static string MimeFor(string file) => Path.GetExtension(file).ToLowerInvariant() switch
    {
        ".png" => "image/png",
        ".jpg" or ".jpeg" or ".jfif" => "image/jpeg",
        ".gif" => "image/gif",
        ".bmp" => "image/bmp",
        ".webp" => "image/webp",
        ".svg" => "image/svg+xml",
        ".ico" => "image/x-icon",
        ".avif" => "image/avif",
        ".tif" or ".tiff" => "image/tiff",
        _ => "application/octet-stream",
    };

    static void OpenExternal(string uri)
    {
        if (Uri.TryCreate(uri, UriKind.Absolute, out var u) && (u.Scheme == Uri.UriSchemeHttps || u.Scheme == Uri.UriSchemeHttp))
            Process.Start(new ProcessStartInfo(u.AbsoluteUri) { UseShellExecute = true });
    }

    static byte[]? ReadResource(string name)
    {
        using var s = typeof(MainForm).Assembly.GetManifestResourceStream(name);
        if (s == null) return null;
        using var m = new MemoryStream(); s.CopyTo(m); return m.ToArray();
    }

    static string ReadResourceText(string name) => Encoding.UTF8.GetString(ReadResource(name) ?? Array.Empty<byte>());

    static Icon? LoadIcon()
    {
        var s = typeof(MainForm).Assembly.GetManifestResourceStream("fotofono.ico");
        return s == null ? null : new Icon(s);
    }

    [DllImport("dwmapi.dll")]
    static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int value, int size);

    void StyleTitleBar()
    {
        int dark = 1, caption = 0x00181412 /* #121418 as COLORREF */, text = 0x00ECE7E4 /* #e4e7ec */;
        DwmSetWindowAttribute(Handle, 20, ref dark, sizeof(int));    // DWMWA_USE_IMMERSIVE_DARK_MODE
        DwmSetWindowAttribute(Handle, 35, ref caption, sizeof(int)); // DWMWA_CAPTION_COLOR (Windows 11)
        DwmSetWindowAttribute(Handle, 36, ref text, sizeof(int));    // DWMWA_TEXT_COLOR (Windows 11)
    }
}

namespace Fotofono;

static class Program
{
    [STAThread]
    static void Main(string[] args)
    {
        // An unexpected error never closes the app silently: it is logged and shown
        Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
        Application.ThreadException += (_, e) => Report(e.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, e) => { if (e.ExceptionObject is Exception ex) Log(ex); };

        ApplicationConfiguration.Initialize();
        // "Open with Fotofono", or an image dropped on the executable's icon
        var start = args.FirstOrDefault(File.Exists);
        Application.Run(new MainForm(start));
    }

    static void Report(Exception ex)
    {
        Log(ex);
        MessageBox.Show(Strings.Get("unexpected") + "\n\n" + ex.Message + "\n\n" +
                        Strings.Get("details") + " " + Path.Combine(MainForm.DataDir, "errors.log"),
                        "Fotofono", MessageBoxButtons.OK, MessageBoxIcon.Error);
    }

    public static void Log(Exception ex)
    {
        try
        {
            Directory.CreateDirectory(MainForm.DataDir);
            File.AppendAllText(Path.Combine(MainForm.DataDir, "errors.log"), $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {ex}\n\n");
        }
        catch { }
    }
}

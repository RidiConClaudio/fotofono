using System.Globalization;

namespace Fotofono;

/// <summary>
/// Texts shown by the Windows shell itself (splash, dialogs, errors).
/// Everything else is translated inside the page (docs/index.html, I18N).
/// The language follows the page's choice; before the page loads it uses the last choice or the Windows language.
/// </summary>
static class Strings
{
    static readonly Dictionary<string, Dictionary<string, string>> T = new()
    {
        ["en"] = new()
        {
            ["splash"] = "FOTOFONO  ·  starting the engine…",
            ["noWebView"] = "Fotofono needs Microsoft Edge WebView2, which doesn't seem to be installed.\n\nOpen Microsoft's download page?",
            ["engineFail"] = "Fotofono couldn't start its page engine:",
            ["unexpected"] = "An unexpected error occurred:",
            ["details"] = "Details are in",
            ["saveTitle"] = "Save the sound",
            ["wav"] = "WAV audio",
            ["zip"] = "ZIP archive",
            ["midi"] = "MIDI file",
        },
        ["it"] = new()
        {
            ["splash"] = "FOTOFONO  ·  avvio del motore…",
            ["noWebView"] = "Fotofono usa Microsoft Edge WebView2, che non risulta installato.\n\nVuoi aprire la pagina di download di Microsoft?",
            ["engineFail"] = "Fotofono non riesce ad avviare il motore della pagina:",
            ["unexpected"] = "Si è verificato un errore imprevisto:",
            ["details"] = "I dettagli sono in",
            ["saveTitle"] = "Salva il suono",
            ["wav"] = "Audio WAV",
            ["zip"] = "Archivio ZIP",
            ["midi"] = "File MIDI",
        },
        ["es"] = new()
        {
            ["splash"] = "FOTOFONO  ·  iniciando el motor…",
            ["noWebView"] = "Fotofono necesita Microsoft Edge WebView2, que no parece estar instalado.\n\n¿Abrir la página de descarga de Microsoft?",
            ["engineFail"] = "Fotofono no pudo iniciar el motor de la página:",
            ["unexpected"] = "Se produjo un error inesperado:",
            ["details"] = "Los detalles están en",
            ["saveTitle"] = "Guardar el sonido",
            ["wav"] = "Audio WAV",
            ["zip"] = "Archivo ZIP",
            ["midi"] = "Archivo MIDI",
        },
        ["fr"] = new()
        {
            ["splash"] = "FOTOFONO  ·  démarrage du moteur…",
            ["noWebView"] = "Fotofono a besoin de Microsoft Edge WebView2, qui ne semble pas installé.\n\nOuvrir la page de téléchargement de Microsoft ?",
            ["engineFail"] = "Fotofono n'a pas pu démarrer le moteur de la page :",
            ["unexpected"] = "Une erreur inattendue s'est produite :",
            ["details"] = "Les détails se trouvent dans",
            ["saveTitle"] = "Enregistrer le son",
            ["wav"] = "Audio WAV",
            ["zip"] = "Archive ZIP",
            ["midi"] = "Fichier MIDI",
        },
        ["de"] = new()
        {
            ["splash"] = "FOTOFONO  ·  Klangmaschine startet…",
            ["noWebView"] = "Fotofono benötigt Microsoft Edge WebView2, das anscheinend nicht installiert ist.\n\nDownloadseite von Microsoft öffnen?",
            ["engineFail"] = "Fotofono konnte die Seiten-Engine nicht starten:",
            ["unexpected"] = "Ein unerwarteter Fehler ist aufgetreten:",
            ["details"] = "Details stehen in",
            ["saveTitle"] = "Klang speichern",
            ["wav"] = "WAV-Audio",
            ["zip"] = "ZIP-Archiv",
            ["midi"] = "MIDI-Datei",
        },
        ["pt"] = new()
        {
            ["splash"] = "FOTOFONO  ·  iniciando o motor…",
            ["noWebView"] = "O Fotofono precisa do Microsoft Edge WebView2, que não parece estar instalado.\n\nAbrir a página de download da Microsoft?",
            ["engineFail"] = "O Fotofono não conseguiu iniciar o motor da página:",
            ["unexpected"] = "Ocorreu um erro inesperado:",
            ["details"] = "Os detalhes estão em",
            ["saveTitle"] = "Salvar o som",
            ["wav"] = "Áudio WAV",
            ["zip"] = "Arquivo ZIP",
            ["midi"] = "Arquivo MIDI",
        },
    };

    static readonly string LangFile = Path.Combine(MainForm.DataDir, "language.txt");

    public static string Lang { get; private set; } = Detect();

    public static string Get(string key) =>
        T.TryGetValue(Lang, out var d) && d.TryGetValue(key, out var s) ? s : T["en"][key];

    /// <summary>Called when the page reports the language the user picked.</summary>
    public static void Set(string? lang)
    {
        if (lang == null || !T.ContainsKey(lang) || lang == Lang) return;
        Lang = lang;
        try { Directory.CreateDirectory(MainForm.DataDir); File.WriteAllText(LangFile, lang); } catch { }
    }

    static string Detect()
    {
        try { var saved = File.ReadAllText(LangFile).Trim(); if (T.ContainsKey(saved)) return saved; } catch { }
        var os = CultureInfo.CurrentUICulture.TwoLetterISOLanguageName;
        return T.ContainsKey(os) ? os : "en";
    }
}

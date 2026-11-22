//! Texts shown by the Windows shell itself (splash, dialogs, errors).
//! Everything else is translated inside the page (docs/index.html, I18N).
//! The language follows the page's choice; before the page loads it uses the last choice or the Windows language.

use std::sync::{LazyLock, RwLock};

const LANGS: [&str; 6] = ["en", "it", "es", "fr", "de", "pt"];

/// One row per key, one column per language, in the order of `LANGS`.
const T: &[(&str, [&str; 6])] = &[
    ("splash", [
        "FOTOFONO  ·  starting the engine…",
        "FOTOFONO  ·  avvio del motore…",
        "FOTOFONO  ·  iniciando el motor…",
        "FOTOFONO  ·  démarrage du moteur…",
        "FOTOFONO  ·  Klangmaschine startet…",
        "FOTOFONO  ·  iniciando o motor…",
    ]),
    ("noWebView", [
        "Fotofono needs Microsoft Edge WebView2, which doesn't seem to be installed.\n\nOpen Microsoft's download page?",
        "Fotofono usa Microsoft Edge WebView2, che non risulta installato.\n\nVuoi aprire la pagina di download di Microsoft?",
        "Fotofono necesita Microsoft Edge WebView2, que no parece estar instalado.\n\n¿Abrir la página de descarga de Microsoft?",
        "Fotofono a besoin de Microsoft Edge WebView2, qui ne semble pas installé.\n\nOuvrir la page de téléchargement de Microsoft ?",
        "Fotofono benötigt Microsoft Edge WebView2, das anscheinend nicht installiert ist.\n\nDownloadseite von Microsoft öffnen?",
        "O Fotofono precisa do Microsoft Edge WebView2, que não parece estar instalado.\n\nAbrir a página de download da Microsoft?",
    ]),
    ("engineFail", [
        "Fotofono couldn't start its page engine:",
        "Fotofono non riesce ad avviare il motore della pagina:",
        "Fotofono no pudo iniciar el motor de la página:",
        "Fotofono n'a pas pu démarrer le moteur de la page :",
        "Fotofono konnte die Seiten-Engine nicht starten:",
        "O Fotofono não conseguiu iniciar o motor da página:",
    ]),
    ("unexpected", [
        "An unexpected error occurred:",
        "Si è verificato un errore imprevisto:",
        "Se produjo un error inesperado:",
        "Une erreur inattendue s'est produite :",
        "Ein unerwarteter Fehler ist aufgetreten:",
        "Ocorreu um erro inesperado:",
    ]),
    ("details", [
        "Details are in",
        "I dettagli sono in",
        "Los detalles están en",
        "Les détails se trouvent dans",
        "Details stehen in",
        "Os detalhes estão em",
    ]),
    ("saveTitle", [
        "Save the sound",
        "Salva il suono",
        "Guardar el sonido",
        "Enregistrer le son",
        "Klang speichern",
        "Salvar o som",
    ]),
    ("wav", ["WAV audio", "Audio WAV", "Audio WAV", "Audio WAV", "WAV-Audio", "Áudio WAV"]),
    ("zip", ["ZIP archive", "Archivio ZIP", "Archivo ZIP", "Archive ZIP", "ZIP-Archiv", "Arquivo ZIP"]),
    ("midi", ["MIDI file", "File MIDI", "Archivo MIDI", "Fichier MIDI", "MIDI-Datei", "Arquivo MIDI"]),
];

static LANG: LazyLock<RwLock<usize>> = LazyLock::new(|| RwLock::new(detect()));

fn lang_file() -> std::path::PathBuf {
    crate::data_dir().join("language.txt")
}

fn index_of(lang: &str) -> Option<usize> {
    LANGS.iter().position(|l| *l == lang)
}

pub fn get(key: &str) -> &'static str {
    let i = *LANG.read().unwrap();
    T.iter().find(|(k, _)| *k == key).map(|(_, v)| v[i]).unwrap_or("")
}

/// Called when the page reports the language the user picked.
pub fn set(lang: &str) {
    let Some(i) = index_of(lang) else { return };
    let mut cur = LANG.write().unwrap();
    if *cur == i {
        return;
    }
    *cur = i;
    let _ = std::fs::create_dir_all(crate::data_dir());
    let _ = std::fs::write(lang_file(), lang);
}

fn detect() -> usize {
    if let Ok(saved) = std::fs::read_to_string(lang_file()) {
        if let Some(i) = index_of(saved.trim()) {
            return i;
        }
    }
    index_of(&os_ui_language()).unwrap_or(0)
}

/// Two-letter code of the Windows display language (like CultureInfo.CurrentUICulture).
#[cfg(windows)]
fn os_ui_language() -> String {
    use windows::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
    let mut buf = [0u16; 85];
    let n = unsafe { LCIDToLocaleName(GetUserDefaultUILanguage() as u32, Some(&mut buf), 0) };
    if n <= 1 {
        return String::new();
    }
    let name = String::from_utf16_lossy(&buf[..(n - 1) as usize]);
    name.split('-').next().unwrap_or("").to_ascii_lowercase()
}

/// Two-letter code of the display language on Linux and macOS.
#[cfg(not(windows))]
fn os_ui_language() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let trimmed = val.trim();
            if !trimmed.is_empty() && trimmed != "C" && trimmed != "POSIX" {
                let code = trimmed
                    .split(['_', '-', '.'])
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if code.len() == 2 {
                    return code;
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    if let Ok(out) = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLocale"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            let code = s
                .trim()
                .split(['_', '-'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if code.len() == 2 {
                return code;
            }
        }
    }
    String::from("en")
}

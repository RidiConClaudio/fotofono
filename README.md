<p align="center"><img src="docs/icon.png" width="96" alt=""></p>

<h1 align="center">Fotofono</h1>

<p align="center"><b>Turn any image into sound — with rules you can read, not a neural network.</b><br>
English · Italiano · Español · Français · Deutsch · Português</p>

<p align="center"><a href="https://ridiconclaudio.github.io/fotofono/"><b>▶ Try it in your browser</b></a> · <a href="../../releases/latest"><b>⬇ Download for Windows / Linux / macOS</b></a></p>

![Fotofono playing a sunset as an inverse spectrogram](docs/screenshot.png)

Fotofono reads an image in nine different, fully deterministic ways and turns each one into stereo audio — and, where there are notes, into a MIDI file you can open in any music software. Every pixel has a consequence you can trace: the same image always gives the same sound, and the manual inside the app tells you exactly which property of the image drives which property of the sound.

It runs as a **native desktop app** on **Windows**, **Linux**, and **macOS** (single portable binary, offline fonts, native *Save as* dialog, *Open with* file argument) or as a **single web page** you can open in any modern browser.

## The nine modes

| Mode | Family | How the image becomes sound |
|---|---|---|
| **Inverse spectrogram** | Waves | The image is read as a spectrogram: each row is a sine oscillator, brightness is its volume. On the Spectrum screen the image reappears. |
| **Wavetable** | Waves | Each row's brightness profile is one cycle of a waveform; the sound morphs from row to row. |
| **Bytes as samples** | Binary | The raw pixel bytes (or the compressed file itself) become the audio samples, in a reading order you choose (rows, serpentine, spiral, Hilbert curve…). |
| **Grid music box** | Pixels | Columns are steps, rows are scale degrees. The cells that differ from the background play — a drawing on white paper plays its lines, not the paper. |
| **Colour melody** | Colour | Hue picks the scale degree, lightness the octave; colourless areas play by brightness; background cells are rests. |
| **Three RGB voices** | Colour | Red, green and blue each follow the centre of their strongest band, as three melodies in three stereo positions. |
| **Colour chord** | Colour | The 12 hue sectors map to the 12 semitones; the most present colours form a sustained chord. |
| **Drums from edges** | Shape | A Sobel filter finds edges: top band → hi-hat, middle → snare, bottom → kick. Column colour picks the bass note. |
| **XY oscilloscope** | Shape | The edges become a stereo signal (left = X, right = Y) that redraws the image on the XY screen. |

<p align="center">
  <img src="docs/screenshot-xy.png" width="49%" alt="XY oscilloscope mode redrawing the word SOUND">
  <img src="docs/screenshot-rgb.png" width="49%" alt="Three RGB voices following the colours of a geometric image">
</p>

## Download and use

**Windows:** download `Fotofono.exe` from the [latest release](../../releases/latest) and double-click it. Nothing to install: it's a single portable file. It needs Microsoft Edge WebView2, which is already part of Windows 11 (on older systems the app offers the download link).

**Linux (CachyOS, Arch, Ubuntu, Debian, Mint, Fedora, Bazzite…):** download `Fotofono-linux-x86_64.tar.gz`, extract `Fotofono-linux-x86_64` and run it (`chmod +x Fotofono-linux-x86_64 && ./Fotofono-linux-x86_64`). Requires `webkit2gtk-4.1` and `gtk3` (works natively on both Wayland and X11).

**macOS (Apple Silicon & Intel):** download `Fotofono-macos-universal.zip`, extract `Fotofono.app` and open it. Uses the built-in macOS `WKWebView` engine.

Settings, logs and the WebView cache live in:
- **Windows:** `%LOCALAPPDATA%\you_are_incredible\Fotofono\`
- **Linux:** `~/.config/you_are_incredible/Fotofono/`
- **macOS:** `~/Library/Application Support/you_are_incredible/Fotofono/`

**Any system:** use the [web version](https://ridiconclaudio.github.io/fotofono/), or open [`docs/index.html`](docs/index.html) in a modern browser.

- Drag, paste (Ctrl+V / Cmd+V) or open (Ctrl+O / Cmd+O) an image, or start from the built-in examples.
- Pick a mode with the buttons on the left or the keys **1–9**; **Space** plays and pauses.
- Turn the knobs: drag up or down, use the mouse wheel or the arrow keys; double-click resets a knob. The essential controls are always visible; **Advanced** shows the rest.
- **WAV** (Ctrl+S / Cmd+S) saves the current mode's sound; **MIDI** (Ctrl+M / Cmd+M) saves its notes; **ZIP** (Ctrl+Shift+S / Cmd+Shift+S) saves the sounds and MIDI files of all nine modes.
- In the desktop app you can also pass an image path to the executable (`Fotofono image.png`) or use *Open with → Fotofono*.

The interface follows your system language and can be changed from the menu at the bottom left.

## MIDI export

Every mode that plays notes can also save them as a standard MIDI file (format 1, one track per voice, with the mode's tempo), ready for a DAW or a notation program:

| Mode | What goes into the MIDI file |
|---|---|
| Grid music box | every note of the roll |
| Colour melody | the melody, plus the bass on its own track |
| Colour chord | the chords, tied across strips when a note continues |
| Drums from edges | kick, snare and hi-hat on the General MIDI drum channel (10), plus the bass |
| Three RGB voices | three tracks, the glides rounded to the nearest semitone |
| Inverse spectrogram | a piano roll of the image (bright areas become notes) |
| Wavetable | the sustained note |

*Bytes as samples* and *XY oscilloscope* produce no notes, so their MIDI button is off.

## Languages

English, Italiano, Español, Français, Deutsch, Português.

To add a language, copy the `en` block of the `I18N` object in `docs/index.html`, translate the values and give it the two-letter language code. Missing keys fall back to English. For the desktop dialogs, add a column to the table in `windows/src/strings.rs`.

## Build the desktop app (Windows, Linux, macOS)

Requirements: [Rust](https://rustup.rs/).
- **Linux** also needs the GTK3 and WebKit2GTK 4.1 development headers:
  - **CachyOS / Arch / Manjaro / EndeavourOS:** `sudo pacman -S --needed webkit2gtk-4.1 gtk3 base-devel pkgconf`
  - **Ubuntu / Debian / Linux Mint:** `sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev pkg-config build-essential`
  - **Fedora / Bazzite / Nobara:** `sudo dnf install gtk3-devel webkit2gtk4.1-devel libsoup3-devel pkgconf-pkg-config gcc`

```bash
cd windows
cargo build --release
```

The result is `windows/target/release/Fotofono.exe` on Windows or `windows/target/release/Fotofono` on Linux and macOS (a single portable file, about 1.5–2.5 MB). The app embeds `docs/index.html` and all fonts, so web and desktop always share the same code and work 100% offline.

## How it is made

- `docs/index.html` — the whole instrument: image analysis, the nine synthesis engines, the interface and the translations, in plain HTML, CSS and JavaScript with no dependencies. Audio is computed sample by sample and played with the Web Audio API.
- `windows/` — a cross-platform Rust shell ([tao](https://github.com/tauri-apps/tao) + [wry](https://github.com/tauri-apps/wry) + [rfd](https://github.com/PolyMeilex/rfd)) using WebView2 on Windows, WebKitGTK 4.1 on Linux (Wayland & X11) and WKWebView on macOS, adding the native *Save as* dialog, *Open with*, offline fonts and a remembered window position.

## Credits

Fonts: [Chakra Petch](https://github.com/m4rc1e/Chakra-Petch), [Share Tech Mono](https://fonts.google.com/specimen/Share+Tech+Mono) and [IBM Plex Sans](https://github.com/IBM/plex), all under the SIL Open Font License 1.1 (see `windows/fonts/`).

## License

[MIT](LICENSE) © 2026 RidiConClaudio

---

### Italiano

**Fotofono trasforma qualsiasi immagine in suono con regole leggibili, senza reti neurali.** Legge l'immagine in nove modi deterministici (spettrogramma, byte grezzi, carillon, colori, wavetable, contorni, oscilloscopio XY…) e ne ricava audio stereo e, dove ci sono note, un file MIDI.

È disponibile come applicazione desktop nativa per **Windows** (`Fotofono.exe`), **Linux** (`Fotofono-linux-x86_64`, compatibile con Wayland e X11 su CachyOS, Arch, Ubuntu, Fedora…) e **macOS** (`Fotofono.app` universale Apple Silicon + Intel), oppure direttamente nella [versione web](https://ridiconclaudio.github.io/fotofono/). L'interfaccia segue la lingua del sistema e si cambia dal menu in basso a sinistra.

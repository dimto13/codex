<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you want the desktop app experience, run <code>codex app</code> or visit <a href="https://chatgpt.com/codex?app-landing-page=true">the Codex App page</a>.
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---
# Meine Anforderungen

- [x] Aren verwendet vollständig getrennte Zustands- und Konfigurationspfade:
  1. `utils/home-dir`: Standardpfad `~/.aren`, `AREN_HOME` und `AREN_SQLITE_HOME`.
  2. `login/auth/storage`: Keyring-Dienste `Aren Auth` und `aren`.
  3. `app-server-transport` / `app-server-daemon`: Socket `aren.sock` sowie
     PID-Dateien `aren.pid` und `aren-updater.pid`.
  4. Projektkonfiguration, Hooks, Regeln und Skills unter `.aren/`.
  5. `core-plugins`: Cache-Pfad `aren-runtimes`.

- [x] Standard-Modellauswahl: `gemma4:e4b` mit hohem Reasoning-Aufwand.
- [x] Standard-Berechtigungen: voller Zugriff ohne Sandbox und Rückfragen.
- [x] Ein einfacher Aufruf von `aren` aktiviert automatisch OSS, Ollama und die
  oben genannten Standardwerte; zusätzliche Startparameter sind nicht nötig.
- [x] Allgemeine lokale Anfragen zu Dateien, Systemzustand und Git werden mit
  den vorhandenen Shell-Werkzeugen gelöst; große MCP-/App-Werkzeugkataloge
  werden pro Anfrage relevant gefiltert statt durch Einzelfall-Skripte ersetzt.

## Vorläufiger Aren-Releaseprozess

- GitHub Actions ist der primäre Release-Builder. Release-Kompilierung läuft
  weder auf dem lokalen Entwicklungsrechner noch auf dem Jenkins-NAS.
- Ein Tag im Format `aren-v*` baut mit begrenzter Cargo-Parallelität zunächst
  Linux x86_64 und veröffentlicht Binary, Archiv, Build-Info und SHA-256 als
  dauerhafte GitHub-Release-Dateien.
- Normale Branch-Pushes lösen keinen Release aus. Manuelle Workflowläufe bauen
  nur Testartefakte und veröffentlichen kein Release.
- Windows ist als nächster Zieltyp vorgesehen und soll auf einem nativen
  GitHub-hosted Windows Runner gebaut und geprüft werden.
- Lokal erfolgen ressourcenschonende, gezielte Crate-Tests. Umfangreiche
  Testmatrizen werden in getrennte CI-Jobs aufgeteilt, statt ungebremst den
  gesamten Workspace parallel zu linken.
- Nach Veröffentlichung wird das Release lokal installiert und mit der
  wiederholbaren Quick-/Full-Live-Qualitätssuite einschließlich Chrome MCP
  geprüft.

## OFFEN: Home-Rebrand `.codex` → `.aren` noch nicht wirksam

> Vermerkt 2026-07-23. In einer späteren Session vollständig auflösen/migrieren.
> Punkt 1 der Anforderungen oben ist erst im Quellcode umgesetzt, **nicht** in den
> laufenden Binaries.

**Ist-Zustand (empirisch verifiziert):**
- Beide `codex`-Binaries lösen ihr Home real auf `~/.codex` auf (`codex doctor` →
  `CODEX_HOME → ~/.codex`):
  - PATH `/usr/local/bin/codex` → Symlink auf `@openai/codex` (node_modules, JS-Wrapper)
  - lokaler Build `codex-rs/target/release/codex` (22.07.2026): Binary enthält 402× `.codex`
    vs. 7× `.aren`, Fehlertext noch „CODEX_HOME points to".
- `AREN_HOME` ist **nicht** gesetzt.
- Aktives Config-Home = `~/.codex/` (config.toml, auth.json, Trust-Levels,
  `[mcp_servers.*]` inkl. `executor` und `chrome-devtools`).
- `~/.aren/` existiert, aber nur teilbefüllt: `mcp-oauth-locks/`, `proxy/`, `tmp/` —
  **keine** config.toml, **keine** auth.json.

**Ursache:** Quellcode ist schon rebranded (`codex-rs/utils/home-dir/src/lib.rs`:
`find_codex_home()` defaultet auf `~/.aren`, Override `AREN_HOME`), aber die
installierten/gebauten Binaries stammen noch aus dem `.codex`-Stand. Einzelne
neuere Subsysteme (MCP-OAuth-Locks, Proxy, Temp) schreiben bereits nach `~/.aren`
→ daher der halb-gefüllte Ordner.

**Zu tun (später):**
1. Richtung festlegen: konsequent auf `.aren` migrieren **oder** Rebrand im Code zurückdrehen.
2. Bei Migration: `~/.codex/*` (config.toml, auth.json, Trust-Levels, MCP-Server)
   nach `~/.aren` übernehmen; übergangsweise `export AREN_HOME=$HOME/.codex`.
3. Binaries aus aktuellem Source neu bauen/installieren, damit PATH-`codex` und
   lokaler Build dasselbe Home nutzen.
4. Klären, ob PATH-`codex` (openai/codex node_modules) durch den aren-Build ersetzt wird.

---
## Quickstart

### Installing and running Codex CLI

Run the following on Mac or Linux to install Codex CLI:

```shell
curl -fsSL https://chatgpt.com/codex/install.sh | sh
```

Run the following on Windows to install Codex CLI:

```shell
powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 | iex"
```

Codex CLI can also be installed via the following package managers:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Business, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).

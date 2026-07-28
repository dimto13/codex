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
- [x] GitHub Actions ist für dieses Repository vollständig deaktiviert. Builds,
  Tests und Veröffentlichungen sollen künftig über Jenkins laufen.

## Geplanter Aren-Releaseprozess

- Die geerbten GitHub-Workflows bleiben als Upstream-Referenz im Repository,
  werden durch die repositoryweite Actions-Sperre aber nicht ausgeführt.
- Die vorhandenen GitHub Releases bleiben verfügbar. Neue Releases können
  unabhängig von GitHub Actions durch Jenkins oder manuell über die GitHub-API
  einschließlich Binary, Archiv, Build-Info und SHA-256 veröffentlicht werden.
- Jenkins soll künftig Build, Tests, Paketierung, Prüfsummen, Smoke-Tests und
  die Veröffentlichung unveränderlicher Tags im Format `aren-v*` übernehmen.
- Der Jenkins-Releasepfad ist noch nicht eingerichtet oder im realen
  Ausführungskontext verifiziert. Bis dahin gibt es bewusst keine automatische
  CI/CD- oder Release-Pipeline.

## Verifizierter Aren-Stand

- `aren-v0.1.1` ist als unveränderlicher GitHub Release veröffentlicht und unter
  `~/.local/lib/aren/aren-v0.1.1/aren` installiert. Der atomare Symlink
  `~/.local/bin/aren` aktiviert diese Version; `aren-v0.1.0` bleibt als lokaler
  Rollback erhalten.
- Aren verwendet real `~/.aren`. `config.toml` und `auth.json` wurden mit den
  restriktiven Rechten `0600` übernommen, das Verzeichnis selbst verwendet
  `0700`. Die offizielle `codex`-Installation bleibt getrennt und unverändert.
- Chrome DevTools MCP läuft headless mit einem isolierten temporären Profil.
  Dadurch können Aren, Codex und wiederholte Qualitätstests parallel arbeiten,
  ohne sich am Standard-Chrome-Profil zu blockieren.
- Die Quick- und Full-Live-Suite wurde am 24. Juli 2026 mit dem installierten
  Release erfolgreich ausgeführt. Die Full-Suite prüfte IANA, Datum/Zeitzone
  für Berlin und den aktuellen offiziellen Rust-Release über Chrome.
- `aren-v0.1.0` bleibt als erster nachvollziehbarer Release bestehen. Der
  verpflichtende Live-Test fand dort eine abgebrochene Chrome-Freigabe; der
  selektiv abgesicherte Fix wurde deshalb als Patch-Release `aren-v0.1.1`
  veröffentlicht, statt den vorhandenen Tag nachträglich zu verändern.

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

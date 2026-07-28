<p align="center"><strong>Aren</strong> is a local-first coding agent powered by Ollama.</p>

Aren is based on the open-source Codex CLI but keeps its configuration,
credentials, sessions and local-model defaults separate from an official Codex
installation.

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
- [x] `/permissions` wirkt auch bei lokalen Modellen ohne künstliche
  „Soll ich fortfahren?“- oder `go`-Zwischenstopps weiter.
- [x] Ein einfacher Aufruf von `aren` aktiviert automatisch OSS, Ollama und die
  oben genannten Standardwerte; zusätzliche Startparameter sind nicht nötig.
- [x] Aren kann Ollama wahlweise lokal oder über
  `AREN_OLLAMA_BASE_URL` auf einem leistungsfähigeren Rechner im LAN nutzen.
- [x] Allgemeine lokale Anfragen zu Dateien, Systemzustand und Git werden mit
  den vorhandenen Shell-Werkzeugen gelöst; große MCP-/App-Werkzeugkataloge
  werden pro Anfrage relevant gefiltert statt durch Einzelfall-Skripte ersetzt.
- [x] GitHub Actions führt nur eine kleine, bereits bewährte CI-Prüfung aus:
  Blob-Größe, `cargo-deny`, Rust-Format/Benchmark-Smoke und `cargo shear`.
  Release-Builds laufen ausschließlich für explizite `aren-v*`-Tags.
- [ ] Versionierte, selbstaktualisierende Releasepakete für Linux x86_64,
  Linux ARM64 und Windows x86_64 sind auf ihren realen GitHub-Runnern geprüft.

# Installation

## Linux

Unterstützt werden Linux x86_64 und Linux ARM64:

```shell
mkdir -p "$HOME/.local/bin"
curl -fsSL \
  https://github.com/dimto13/codex/releases/latest/download/aren-update \
  -o "$HOME/.local/bin/aren-update"
chmod 0755 "$HOME/.local/bin/aren-update"
export PATH="$HOME/.local/bin:$PATH"
aren-update
```

Trage `~/.local/bin` dauerhaft in den `PATH` deiner Shell ein.

## Windows

In PowerShell auf Windows x86_64:

```powershell
$installDir = Join-Path $HOME ".local\bin"
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Invoke-WebRequest `
  https://github.com/dimto13/codex/releases/latest/download/aren-update.ps1 `
  -OutFile (Join-Path $installDir "aren-update.ps1")
Invoke-WebRequest `
  https://github.com/dimto13/codex/releases/latest/download/aren-update.cmd `
  -OutFile (Join-Path $installDir "aren-update.cmd")
& (Join-Path $installDir "aren-update.ps1")
```

Füge anschließend `$HOME\.local\bin` dauerhaft zum Benutzer-`PATH` hinzu.

## Erster Start

Bei lokaler Inferenz muss Ollama auf demselben Rechner laufen und das
Standardmodell vorhanden sein:

```shell
ollama pull gemma4:e4b
aren
```

## Ollama auf einem anderen Rechner im LAN

Auf dem Ollama-Rechner muss der Dienst auf einer LAN-Adresse lauschen. Unter
Linux mit systemd kann dafür beispielsweise `systemctl edit ollama.service`
verwendet werden:

```ini
[Service]
Environment="OLLAMA_HOST=0.0.0.0:11434"
```

Danach den Dienst neu starten, `gemma4:e4b` dort installieren und Port 11434 in
der Firewall ausschließlich für das vertrauenswürdige lokale Netz freigeben.
Auf einem Linux- oder macOS-Rechner mit Aren genügt dann:

```shell
export AREN_OLLAMA_BASE_URL="http://192.168.1.25:11434"
aren
```

Unter Windows kann die Adresse für den aktuellen PowerShell-Prozess oder
dauerhaft für das Benutzerkonto gesetzt werden:

```powershell
$env:AREN_OLLAMA_BASE_URL = "http://192.168.1.25:11434"
[Environment]::SetEnvironmentVariable(
  "AREN_OLLAMA_BASE_URL",
  "http://192.168.1.25:11434",
  "User"
)
aren
```

Aren akzeptiert auch `192.168.1.25:11434` oder eine bereits mit `/v1`
abgeschlossene URL. `OLLAMA_HOST` wird als Fallback ebenfalls erkannt. Für eine
dauerhafte Einrichtung unter Linux oder macOS wird `AREN_OLLAMA_BASE_URL` in
das Shell-Profil eingetragen. Ollamas HTTP-Schnittstelle bietet selbst keine
Transportverschlüsselung; außerhalb eines vertrauenswürdigen LANs sollte sie
nur über einen abgesicherten Reverse-Proxy oder VPN erreichbar sein. Weitere
Hinweise stehen in Ollamas
[Netzwerk-Dokumentation](https://docs.ollama.com/faq#how-can-i-expose-ollama-on-my-network).

Ein späteres Update installiert automatisch das neueste stabile Release:

```shell
aren update
```

MCP-Server und persönliche Einstellungen werden absichtlich nicht als
Releasebestandteil verteilt. Sie liegen getrennt unter `~/.aren/` und müssen
pro Rechner eingerichtet werden. Insbesondere muss Executor auf dem Zielrechner
installiert und dort als MCP-Server konfiguriert sein. Zugangsdaten wie
`auth.json` sollten nicht unkontrolliert zwischen Rechnern kopiert werden.

# Releaseprozess

- Die geerbten Upstream-Workflows bleiben als Referenz im Repository, sind auf
  GitHub aber deaktiviert.
- Aktiv sind nur `aren-ci`, dessen wiederverwendbare Prüfungen und
  `aren-release`.
- Normale Branch-Pushes erzeugen kein Release. Ein unveränderlicher Tag im
  Format `aren-v*` baut, prüft und veröffentlicht Aren.
- Jedes Paket enthält Aren, den plattformspezifischen Updater, Build-Metadaten
  und eine separat veröffentlichte SHA-256-Prüfsumme.

Ein neues Release wird nach grüner CI aus dem gewünschten Commit erstellt:

```shell
git tag -a aren-v0.1.2 -m "Aren 0.1.2"
git push origin aren-v0.1.2
```

Details, Prüf- und Updatebefehle stehen in
[`docs/aren-releases.md`](docs/aren-releases.md).

# Verifizierter Aren-Stand

- `aren-v0.1.1` wurde am 24. Juli 2026 als erstes vollständig geprüftes
  Linux-x86_64-Release veröffentlicht.
- Aren verwendet real `~/.aren`; die offizielle `codex`-Installation bleibt
  getrennt und unverändert.
- Chrome DevTools MCP läuft headless mit einem isolierten temporären Profil.
- Die Quick- und Full-Live-Suite prüft reale Webrecherche einschließlich
  Quellenvalidierung.

# Entwicklung

- [Build- und Releaseanleitung](docs/aren-releases.md)
- [Allgemeine Build-Anleitung](docs/install.md)
- [Mitwirken](docs/contributing.md)

Aren enthält Code aus dem OpenAI-Codex-Projekt und steht wie das
Ausgangsprojekt unter der [Apache-2.0-Lizenz](LICENSE).

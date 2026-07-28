

###  Ziel

Implementiere im Open-Source-Codex-CLI einen neuen CLI-Modus **`interactive-search`**, der eine **headless Ausführung des interaktiven Agenten mit aktivierter Web-Search** ermöglicht.

---

###  Grundprinzip (verbindlich)

* **KEINE Änderungen an Server-, Netzwerk- oder Sandbox-Policies**
* **KEINE Erweiterung von `exec`**
* **KEINE neue Web-Search-Logik**
* **AUSSCHLIESSLICH Wiederverwendung des bestehenden interaktiven Codepfads**
* Der neue Modus ist **eine gesteuerte interaktive Session**, kein Batch-Exec

---

###  Funktionale Anforderungen

1. **CLI-Interface**

   * Neuer Subcommand:

     ```bash
     codex interactive-search "<PROMPT>"
     ```
   * Optional:

     ```bash
     --json
     --timeout <seconds>
     ```
   * `--search` ist implizit aktiviert

2. **Session-Typ**

   * Nutze **denselben Agent- und Session-Typ wie `codex --search`**
   * Web-Search (`web_search` Tool) muss verfügbar sein
   * Keine Einschränkung auf One-Shot-Reasoning

3. **Prompt-Handling**

   * Das übergebene Prompt wird **automatisch in die Session eingespeist**
   * **Kein Warten auf User-Input**
   * Keine Prompt-Loop-UI

4. **Ausführung**

   * Agent darf:

     * mehrere Web-Search-Aufrufe machen
     * Quellen vergleichen
     * iterativ denken
   * Verhalten muss identisch zum interaktiven Modus sein

5. **Beendigung**

   * Nach dem finalen Assistant-Output:

     * Session sauber schließen
     * Prozess beenden (`exit 0`)
   * Kein Verbleib im REPL

6. **Ausgabe**

   * Standard: vollständiger Text-Output
   * Optional `--json`: strukturierte Ausgabe

     ```json
     {
       "answer": "...",
       "sources": [...],
       "timestamp": "...",
       "model": "..."
     }
     ```

---

###  Technische Leitplanken (wichtig)

* Verwende **denselben Initialisierungs-Code** wie beim interaktiven TUI
* Deaktiviere ausschließlich:

  * TUI-Rendering
  * Keybindings
  * Prompt-Loop
* **NICHT**:

  * Tool-Routing
  * Agent-Logik
  * Web-Search-Berechtigungen

---

###  Interner Ablauf (verbindliches Call-Pattern)

1. CLI erkennt `interactive-search`
2. Initialisiert interaktive Session **ohne UI**
3. Injiziert Prompt programmgesteuert
4. Wartet auf finalen Assistant-Turn
5. Sammelt Output
6. Gibt Ergebnis aus
7. Beendet Session

---

###  Erfolgskriterium (akzeptanzrelevant)

Folgender Befehl muss funktionieren:

```bash
codex interactive-search "What is the current price of XAUUSD?"
```

Erwartetes Verhalten:

* sichtbare Web-Search-Schritte
* Quellenangaben
* zeitbezogene Antwort
* automatisches Beenden

---

###  Explizit nicht Teil des Auftrags

* Keine Garantie auf Datenrichtigkeit
* Kein Ersatz für APIs
* Kein Produktions-SLA
* Keine Sandbox-Eskalation
* Keine Netzwerk-Bypässe

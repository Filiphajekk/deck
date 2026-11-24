# Deckless

[English](README.md) · **Čeština**

**Stream Deck bez Stream Decku.** Stiskneš zkratku (nebo boční tlačítko myši) a přímo u kurzoru se objeví panel tlačítek. Vybereš akci a panel zmizí. Žádný hardware, žádný telefon.

- **Vestavěné akce:** klávesové zkratky, psaní textu, spouštění programů, otevírání webů a složek, příkazy, média, hlasitost (i jednotlivých aplikací), mikrofon, přepínání zvukových zařízení, okna, zamknutí, uspání, výstřižek, OBS.
- **Profily podle aplikace:** v prohlížeči jiná tlačítka než ve VS Code, panel se přepne sám.
- **Pluginy pro Stream Deck:** existující pluginy (Discord, Spotify, Twitch, Home Assistant…) fungují i tady.

Aplikace je zdarma, běží jen lokálně, nemá účty ani telemetrii. Po síti komunikuje jen s localhostem (OBS a pluginy) a s GitHubem, když instaluješ plugin z obchodu.

## Instalace

**Nejrychleji:** spusť **[`Deckless-Setup.exe`](Deckless-Setup.exe)** z hlavní složky (na GitHubu na něj klikni a dej *Download raw file*).

**Z příkazové řádky** (PowerShell):

```powershell
irm https://raw.githubusercontent.com/Filiphajekk/deck/main/install.ps1 | iex
```

Tichá instalace bez okna instalátoru:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/Filiphajekk/deck/main/install.ps1))) -Silent
```

Nebo v obyčejném `cmd`:

```cmd
curl -L -o %TEMP%\Deckless-Setup.exe https://github.com/Filiphajekk/deck/raw/main/Deckless-Setup.exe && %TEMP%\Deckless-Setup.exe
```

Po instalaci se Deckless spustí a usadí se v oznamovací oblasti (vedle hodin). Ve výchozím stavu se spouští po přihlášení. Instaluje se jen pro aktuálního uživatele, správce není potřeba.

Instalátor není digitálně podepsaný, takže Windows SmartScreen může varovat. Klikni na *Další informace* → *Přesto spustit*.

Potřebuješ Windows 10 nebo 11 (x64) s WebView2. Ve Windows 11 je WebView2 předinstalovaný, do Windows 10 ho instalátor případně doinstaluje.

## První kroky

- **Ctrl+Shift+Space** otevře panel u kurzoru.
  - **Krátce ťukni:** panel zůstane otevřený a klikneš na tlačítko. Zavřeš ho klávesou Esc, kliknutím mimo nebo dalším stiskem zkratky.
  - **Podrž:** najeď směrem k tlačítku a pusť. U radiálního panelu stačí švihnout správným směrem.
  - Šipkami se mezi tlačítky dá i pohybovat, Enter tlačítko spustí.
- Ozubené kolo v panelu nebo ikona v oznamovací oblasti otevře **editor**:
  - vlevo jsou profily, uprostřed náhled panelu, vpravo akce,
  - akci přetáhneš do slotu, tlačítka jde přesouvat, pravým tlačítkem duplikuješ, smažeš nebo vytvoříš složku,
  - po kliknutí na tlačítko vpravo nastavíš titulek, ikonu a parametry, tlačítkem **Vyzkoušet** ho rovnou spustíš,
  - všechno se ukládá samo.
- **Přidat profil pro aplikaci** (vlevo dole) vytvoří profil, který se použije, když je daná aplikace v popředí. Prázdná místa se doplní z výchozího profilu.
- V **Nastavení** změníš zkratku, zapneš boční tlačítko myši, upravíš vzhled, jazyk a připojení k OBS.

## Pluginy pro Stream Deck

### Obchod s pluginy

Na kartě **Pluginy → Obchod** je 39 vybraných open-source pluginů, které fungují ve Windows: Microsoft Teams, Google Meet, Spotify, Home Assistant, OBS, HWiNFO, Bitwarden, Docker, VS Code, IDE od JetBrains, SimHub a další. Vyber kategorii nebo hledej a klikni na **Instalovat**. Plugin se stáhne přímo z GitHubu jeho autora (nejnovější verze, nebo ověřená verze, když GitHub nejde načíst). Každý plugin má vlastní licenci.

Katalog je v `src-tauri/store/catalog.json`; `node scripts/store-check.mjs` zkontroluje všechny položky (soubor v release, čitelný manifest s kódem pro Windows).

### Další pluginy

1. Plugin stáhni z [Elgato Marketplace](https://marketplace.elgato.com/stream-deck/plugins). Dostaneš soubor `.streamDeckPlugin`.
2. Nainstaluj ho jedním z těchto způsobů:
   - dvojklikem na soubor,
   - přetažením souboru do okna editoru,
   - na kartě **Pluginy** tlačítkem *Instalovat plugin ze souboru*.
3. Akce pluginu najdeš v editoru v kategorii **Pluginy**. Jejich nastavení (Property Inspector) se zobrazí vpravo po kliknutí na tlačítko.

Většina pluginů je napsaná v Node.js. Pokud Node.js v počítači chybí, editor na to upozorní. Stáhneš ho z [nodejs.org](https://nodejs.org/) (verze 20 nebo novější).

Podporované jsou akce pro klasická tlačítka. Akce určené jen pro otočné ovladače (Stream Deck +) jsou označené jako nepodporované.

## Známá omezení

- **Hry v exkluzivním režimu celé obrazovky:** přes ně se panel nevykreslí. Přepni hru do režimu „okno bez okrajů“ (borderless windowed).
- **Aplikace spuštěné jako správce:** Windows (UIPI) nedovolí běžné aplikaci posílat do nich klávesy. Zapni v Nastavení *Spouštět jako správce*. Windows se pak při každém spuštění zeptá na povolení. Když akce do takového okna selže, Deckless to jednou připomene.
- **Anti-cheat:** některé hry umělý vstup ignorují nebo blokují. Deckless se to nikdy nesnaží obejít.
- **Vlastní pluginy Elgato (např. Discord):** mají zašifrovaný `manifest.json`, aby je načetla jen oficiální aplikace Stream Deck. Deckless je nainstalovat neumí a nikdy se to nesnaží obejít. Pluginů od jiných autorů se to netýká.
- Pluginy pro Stream Deck, které přibalují vlastní profily Stream Decku (`switchToProfile`), tuhle funkci nevyužijí. Akce samotné fungují.

## Data

Všechno je v `%APPDATA%\Deckless`:

```
config.json              nastavení
profiles\<id>.json       profily
plugins\<uuid>.sdPlugin  nainstalované pluginy
plugin-settings\         nastavení pluginů
icons\cache\             ikony aplikací
logs\                    logy (i logy pluginů)
```

Zálohu všeho uděláš v Nastavení tlačítkem *Exportovat vše*.

## Sestavení ze zdrojáků

Potřebuješ:

- Rust (stable, `x86_64-pc-windows-msvc`): `winget install Rustlang.Rustup`
- Visual Studio Build Tools s „Desktop development with C++“
- Node.js 20+ (testovací plugin potřebuje 22+)

```powershell
npm install
npm run tauri dev       # vývojová verze
npm run tauri build     # instalátor NSIS v src-tauri\target\release\bundle\nsis
npm run installer       # totéž a navíc ho zkopíruje do Deckless-Setup.exe v hlavní složce
```

Testy a kontroly:

```powershell
cd src-tauri
cargo test                                   # unit + integrační testy (OBS mock, SDK hostitel)
cargo clippy --all-targets -- -D warnings
cd ..
npm test                                     # Vitest (spouštěj z PowerShellu nebo cmd)
npm run typecheck
npm run lint
npm run format:check
```

Architektura a rozhodnutí: [SPEC.md](SPEC.md), [DECISIONS.md](DECISIONS.md), stav práce: [PROGRESS.md](PROGRESS.md).

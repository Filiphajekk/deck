# Deckless – Stream Deck bez Stream Decku

> Zadání pro Claude Code. Celý dokument si přečti dřív, než napíšeš první řádek kódu.

---

## 0. Jak s tímto zadáním pracovat

- **Pracuj samostatně a na nic se neptej.** Kde zadání něco neřeší, rozhodni sám tak, aby to co nejvíc odpovídalo cíli v kapitole 1. Každé takové rozhodnutí zapiš jednou větou do `DECISIONS.md`.
- **Postupuj po fázích (kapitola 11) v pořadí.** Fáze je hotová, až když projde build, testy a všechna akceptační kritéria. Teprve potom začni další.
- **Na začátku vytvoř `CLAUDE.md`**, aby se v něm v příští session dalo navázat. Stačí pár řádků: odkaz na `SPEC.md`, `PROGRESS.md` a `DECISIONS.md`, příkazy pro build, testy a spuštění.
- **Po každém větším kroku aktualizuj `PROGRESS.md`:** co je hotové, co se dělá teď, co zbývá a známé problémy. Práce se přeruší několikrát a podle tohoto souboru se musí dát kdykoli navázat.
- **Po každé dokončené fázi udělej git commit** se srozumitelnou zprávou.
- Když něco nejde udělat tak, jak je to tu popsané, zvol nejbližší funkční alternativu, zapiš ji do `DECISIONS.md` a pokračuj. Nezastavuj se.
- Platforma je **jen Windows 10/11 (x64)**. Na macOS ani Linux neber ohled.
- Názvy v kódu, komentáře a commity piš anglicky. Uživatelské rozhraní je česky, s anglickým překladem (kapitola 9).

---

## 1. Cíl produktu

Desktopová aplikace pro Windows, která dělá totéž co Elgato Stream Deck, ale **bez jakéhokoli hardwaru a bez telefonu**. Uživatel stiskne zkratku (nebo boční tlačítko myši) a **přímo u kurzoru** se mu objeví panel tlačítek. Vybere akci a panel zmizí.

Musí to být univerzální: na práci, hraní, Discord, streamování, prostě na cokoli, co uživatel používá. Stojí to na třech vrstvách:

1. **Vestavěné akce, které fungují všude:** zkratky, spouštění, text, média, zvuk, okna, OBS (kapitola 6).
2. **Profily podle aktivní aplikace:** panel se sám přizpůsobí programu, který je zrovna v popředí (kapitola 5).
3. **Kompatibilita se Stream Deck SDK:** běží na tom existující pluginy pro Stream Deck (Discord, Spotify, Twitch, Home Assistant…), takže integrace nemusíme psát sami (kapitola 7).

Hlavní pravidlo pro všechna rozhodnutí: **vybrat akci přes panel musí být rychlejší a pohodlnější než hledat ji jinde.** Panel se proto musí objevit okamžitě (< 100 ms), nesmí krást focus a nesmí rozbít práci v aplikaci pod ním.

Aplikace je zdarma, lokální, bez účtů a bez telemetrie. Po síti komunikuje jen s localhostem (OBS WebSocket a pluginy).

> Název „Deckless“ je pracovní. Udržuj ho na jednom místě (konstanta + `tauri.conf.json`), aby šel snadno změnit. V názvu, ikoně ani UI nepoužívej ochranné známky a loga Elgato.

---

## 2. Technologie

| Vrstva | Volba |
|---|---|
| Framework | **Tauri 2** |
| Backend | **Rust** (stable), crate `windows` (oficiální bindingy Microsoftu) pro Win32, COM a WinRT |
| Frontend | **React + TypeScript + Vite**, styly **Tailwind CSS** |
| Ikony v UI | **Lucide** (`lucide-react`) |
| Async / WebSocket | `tokio`, `tokio-tungstenite` |
| Serializace | `serde`, `serde_json` |
| Logování | `tracing` + rotující soubor v `%APPDATA%\Deckless\logs` |
| Tauri pluginy | `single-instance`, `autostart`, `dialog`, `fs`; tray přes vestavěné Tauri 2 API |
| Instalátor | Tauri bundler, **NSIS** |

### 2.1 Příprava prostředí (fáze 0)
Ověř, že jsou nainstalované: Rust (rustup, toolchain `stable-x86_64-pc-windows-msvc`), Visual Studio Build Tools s „Desktop development with C++“, Node.js 20+, WebView2 Runtime (na Windows 11 je předinstalovaný). Co chybí, doinstaluj přes `winget`. Pokud to bez admin práv nejde, napiš do `PROGRESS.md` přesný příkaz, který má uživatel spustit, a pokračuj tím, co jde udělat i bez toho.

---

## 3. Architektura

### 3.1 Rust moduly (`src-tauri/src/`)
- `input/`: low-level hooky klávesnice a myši (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`) v **samostatném vlákně s vlastní smyčkou zpráv**. Callback hooku musí vrátit odpověď okamžitě. Událost jen pošle kanálem dál a nic nezpracovává, jinak Windows hook potichu odpojí. Dál `SendInput` pro posílání kláves a textu.
- `overlay/`: správa overlay okna (zobrazení u kurzoru, multi-monitor, DPI, focus).
- `foreground/`: sledování aktivního okna přes `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`. Zjistí HWND, PID a název exe (`QueryFullProcessImageNameW`).
- `actions/`: engine pro spouštění akcí (kapitola 6).
- `audio/`: Core Audio (hlasitost, mute, zařízení, hlasitost jednotlivých aplikací).
- `media/`: ovládání médií a info o přehrávané skladbě.
- `obs/`: klient pro obs-websocket v5.
- `profiles/`: načítání, ukládání, výběr profilu podle aplikace, migrace verzí.
- `sdk_host/`: hostitel Stream Deck pluginů (kapitola 7).
- `icons/`: extrakce ikon z exe (`SHGetFileInfoW` / `PrivateExtractIconsW` → PNG → data URL), cache na disku.
- `apps/`: seznam nainstalovaných aplikací (zástupci `.lnk` ve Start menu v `%ProgramData%` i `%AppData%`) a běžících procesů.
- `config/`: globální nastavení.

### 3.2 Okna
1. **Overlay:** vytvoř ho jednou při startu jako skryté a pak ho jen zobrazuj a skrývej, protože opakované vytváření je pomalé. Nastavení: `transparent: true`, `decorations: false`, `alwaysOnTop: true`, `skipTaskbar: true`, `focus: false`, `shadow: false`, `resizable: false`. Po vytvoření přes Win32 přidej `WS_EX_NOACTIVATE` a `WS_EX_TOOLWINDOW`. **Overlay nikdy nesmí převzít focus.** Kdyby ho převzal, klávesové zkratky by šly do overlaye místo do aplikace.
2. **Editor / nastavení:** normální okno, otevírá se z tray ikony nebo z overlaye (tlačítko ozubeného kola).
3. **Tray ikona:** menu „Otevřít editor“, „Pozastavit“, „Ukončit“.

### 3.3 Komunikace
Frontend ↔ backend přes Tauri commands a events. Všechny typy sdílej: Rust structy mají `serde`, TS typy drž v `src/types/` a ručně je udržuj ve shodě. Případně použij `ts-rs` na generování, rozhodnutí zapiš.

### 3.4 Data na disku
```
%APPDATA%\Deckless\
  config.json
  profiles\<id>.json
  plugins\<uuid>.sdPlugin\
  plugin-settings\<uuid>.json      (globální a per-button nastavení pluginů)
  icons\cache\
  logs\
```
Každý JSON má pole `version` kvůli budoucím migracím. Zapisuj atomicky: nejdřív do `.tmp`, pak přejmenovat.

---

## 4. Overlay: chování a interakce

### 4.1 Vyvolání
- Výchozí zkratka: **Ctrl+Shift+Space**. V nastavení jde změnit na libovolnou kombinaci.
- Volitelně **boční tlačítko myši** (X1, X2) nebo prostřední tlačítko. Ve výchozím stavu je to vypnuté. Když je zapnuté, hook tlačítko „spolkne“, aby nespustilo akci Zpět v prohlížeči.
- Zkratku detekuj low-level hookem, ne přes `RegisterHotKey`, protože potřebujeme i událost puštění.
- **Dva režimy podle délky stisku:**
  - **Podržení (> 200 ms, nastavitelné):** panel je vidět, dokud uživatel drží. Najede směrem k tlačítku, pustí a akce se provede. Když pustí uprostřed nebo mimo tlačítka, nic se nestane.
  - **Krátké ťuknutí:** panel zůstane otevřený, uživatel klikne myší. Zavře se klávesou Esc, kliknutím mimo panel nebo opětovným stiskem zkratky.
- Protože overlay nemá focus, klávesy Esc, šipky a Enter při otevřeném panelu zachytává keyboard hook a **spolkne je**, aby nedošly do aplikace pod panelem. Šipkami jde mezi tlačítky i navigovat.

### 4.2 Umístění
- Střed panelu je na pozici kurzoru. Když by panel přesahoval okraj monitoru, posuň ho dovnitř.
- Správně musí fungovat více monitorů s různým DPI (per-monitor DPI awareness v2).

### 4.3 Rozložení (každý profil si volí vlastní)
- **Radiální** (výchozí): 8 výsečí kolem středu. Střed = zpět ze složky nebo zavřít. Výběr výseče se počítá **podle úhlu** od středu, ne podle toho, nad kterým prvkem kurzor zrovna je. Funguje tak i rychlé „švihnutí“.
- **Mřížka:** volitelně 2×2 až 8×8.

### 4.4 Vzhled
- Tmavé poloprůhledné pozadí s rozostřením (pokud jde rozostření udělat spolehlivě, jinak jen průhlednost), zaoblené rohy a zvýraznění vybraného tlačítka.
- Tlačítko ukazuje ikonu a krátký titulek. Toggle tlačítka ukazují aktuální stav (např. přeškrtnutý mikrofon).
- Animace otevření a zavření ≤ 120 ms. Musí jít úplně vypnout.
- Nastavitelné: velikost, průhlednost, barva zvýraznění.

### 4.5 Provedení akce (kritické)
1. Při otevření panelu si ulož HWND okna, které bylo v popředí.
2. Po výběru panel **nejdřív skryj**.
3. Pokud se focus mezitím změnil, vrať ho uloženému oknu (`SetForegroundWindow`, případně přes `AttachThreadInput`, když prosté volání nestačí).
4. Počkej ~30 ms, potom spusť akci.

Akceptační test: v Poznámkovém bloku s napsaným textem otevřu panel, vyberu tlačítko s akcí Ctrl+A a text v Poznámkovém bloku se označí.

---

## 5. Profily

- **Výchozí profil** existuje vždy a nejde smazat.
- **Profily aplikací** se přiřazují podle názvu exe (bez ohledu na velká a malá písmena; jeden profil může mít víc exe, např. `chrome.exe`, `msedge.exe`).
- Při otevření panelu se podle exe aplikace v popředí vybere profil. Když žádný nesedí, použije se výchozí.
- Volba profilu „Doplnit prázdná místa z výchozího profilu“ (ve výchozím stavu zapnutá): prázdné sloty se vyplní tlačítky z výchozího profilu.
- **Složky:** tlačítko může otevřít podstránku s vlastními sloty. Hloubka vnoření není omezená, střed nebo tlačítko „zpět“ vrací o úroveň výš.
- Profil jde ručně přepnout akcí `switchProfile`.
- Import a export profilu do souboru `.deckless.json`.

### 5.1 První spuštění
Vytvoř výchozí profil, který je užitečný hned bez nastavování:
přehrát/pauza, další skladba, ztlumit mikrofon (toggle), hlasitost + a −, výstřižek obrazovky (Win+Shift+S), průzkumník souborů, správce úloh (Ctrl+Shift+Esc).

Dál nabídni hotové **startovní profily** pro aplikace nalezené v systému. Použij jen zkratky, které jsou ověřeně výchozí:
- **Prohlížeče** (Chrome, Edge, Firefox, Brave, Opera): nová karta Ctrl+T, znovu otevřít zavřenou kartu Ctrl+Shift+T, zavřít kartu Ctrl+W, anonymní okno (Ctrl+Shift+N pro Chromium, Ctrl+Shift+P pro Firefox), obnovit F5, historie Ctrl+H.
- **VS Code:** paleta příkazů Ctrl+Shift+P, terminál Ctrl+`, hledat v souborech Ctrl+Shift+F, boční panel Ctrl+B, formátovat dokument Shift+Alt+F.
- **OBS:** akce přes OBS WebSocket (kapitola 6), pokud je připojení nastavené.

U dalších aplikací (Discord, Spotify…) v průvodci nabídni instalaci Stream Deck pluginu (kapitola 7), místo vymýšlení zkratek.

---

## 6. Vestavěné akce

Každá akce má: `type`, parametry, volitelně ikonu a titulek. Pokud je toggle, má dva stavy, každý s vlastní ikonou a titulkem.

| Typ | Parametry | Implementace |
|---|---|---|
| `hotkey` | sekvence kombinací kláves | `SendInput`. Správně ošetři modifikátory (pustit vše ve správném pořadí, i když akce selže). Win+L nejde poslat přes SendInput, rozpoznej ho a zavolej `LockWorkStation`. |
| `typeText` | text | `SendInput` s `KEYEVENTF_UNICODE`, ať fungují diakritika a emoji |
| `launch` | cesta, argumenty, pracovní složka, „spustit jako správce“ | `ShellExecuteExW` (verb `runas` pro správce) |
| `open` | URL / soubor / složka / `ms-settings:` URI | `ShellExecuteExW` |
| `command` | příkaz, shell (cmd/PowerShell), skrýt okno | `CreateProcessW` s `CREATE_NO_WINDOW` |
| `media` | playPause / next / previous / stop | virtuální media klávesy přes `SendInput` |
| `nowPlaying` | – | `GlobalSystemMediaTransportControlsSessionManager` (WinRT). Tlačítko ukazuje název skladby a obal alba, klik = play/pause. |
| `volume` | up / down / set (krok %) / muteToggle | `IAudioEndpointVolume` na výchozím výstupu |
| `micMute` | toggle | `IAudioEndpointVolume::SetMute` na výchozím záznamovém zařízení (role `eCommunications`). Funguje univerzálně pro Discord, Teams, hry… Stav zobrazuj živě. |
| `appVolume` | exe, up / down / muteToggle | `IAudioSessionManager2` → `ISimpleAudioVolume` pro sessiony daného procesu |
| `audioDevice` | konkrétní zařízení nebo „cyklovat seznamem“, výstup / vstup | Nedokumentované COM rozhraní `IPolicyConfig` (používají ho SoundSwitch a AudioDeviceCmdlets). GUID a pořadí metod ověř podle těchto open-source implementací. Nastav všechny tři role (console, multimedia, communications). |
| `window` | minimalizovat / maximalizovat / přesunout na další monitor / přepnout „vždy navrchu“ / zavřít | Win32 na uloženém HWND aktivního okna |
| `system` | zamknout / uspat / výstřižek | `LockWorkStation`, `SetSuspendState`, hotkey Win+Shift+S |
| `obs` | přepnout scénu / spustit+zastavit nahrávání / spustit+zastavit stream / ztlumit zdroj / replay buffer uložit | obs-websocket v5 (výchozí `ws://127.0.0.1:4455`, autentizace podle protokolu v5 = SHA256 challenge). Připojuj se líně, při výpadku znovu s backoffem. Stav (nahrává/streamuje) zobrazuj živě. |
| `multi` | seznam akcí + prodlevy mezi nimi | postupné provedení, zrušení při chybě se zaznamená do logu |
| `toggle` | akce pro stav A, akce pro stav B | střídá stavy, každý má vlastní ikonu |
| `folder` | podstránka | otevře podstránku |
| `switchProfile` | id profilu | ruční přepnutí |
| `delay` | ms | jen uvnitř `multi` |
| `plugin` | uuid pluginu, uuid akce, nastavení | kapitola 7 |

Chyby akcí nikdy nesmí shodit aplikaci. Na tlačítku se krátce ukáže červený vykřičník a podrobnosti jdou do logu.

---

## 7. Hostitel Stream Deck pluginů (kompatibilita se Stream Deck SDK)

Cíl: uživatel nainstaluje existující Stream Deck plugin (soubor `.streamDeckPlugin` nebo složku `.sdPlugin`) a jeho akce se objeví v editoru a fungují v panelu.

Implementuj hostitelskou stranu protokolu podle oficiální dokumentace Stream Deck SDK (docs.elgato.com/streamdeck) a pro ověření detailů se podívej na open-source projekt **OpenDeck** (github.com/nekename/OpenDeck), který tentýž protokol implementuje v Tauri a Rustu, a na specifikaci **OpenAction**. Kód z OpenDecku nekopíruj, je pod GPL. Použij ho jen jako referenci chování. Pokud se cokoli níže liší od dokumentace, má přednost dokumentace.

### 7.1 Instalace
- `.streamDeckPlugin` je ZIP. Rozbal ho do `%APPDATA%\Deckless\plugins\<uuid>.sdPlugin`.
- Instalace jde přetažením souboru do editoru nebo tlačítkem „Instalovat plugin“. Zaregistruj i asociaci přípony `.streamDeckPlugin` v instalátoru.
- Načti `manifest.json`: `UUID`, `Name`, `Version`, `CodePath` / `CodePathWin`, `Actions[]` (každá má `UUID`, `Name`, `Icon`, `States[]`, `PropertyInspectorPath`, `Controllers`), `PropertyInspectorPath`, `Nodejs`, `ApplicationsToMonitor`, `SDKVersion`.
- Podporuj jen akce s kontrolerem `Keypad` (nebo bez uvedeného kontroleru). Akce jen pro `Encoder` označ jako nepodporované.
- Plugin bez spustitelného kódu pro Windows označ v editoru jako nekompatibilní.

### 7.2 Spuštění pluginu
- Hostitel spustí lokální WebSocket server na volném portu na `127.0.0.1`.
- Plugin spusť v jeho složce (pracovní adresář = složka pluginu) s argumenty:
  `-port <port> -pluginUUID <uuid> -registerEvent registerPlugin -info <json>`
- Pokud `CodePath` končí `.js` nebo manifest obsahuje `Nodejs`, spusť ho přes `node` ze systému. Když Node chybí, ukaž v editoru srozumitelnou hlášku s odkazem na nodejs.org.
- HTML pluginy (`CodePath` je `.html`) spusť ve skrytém webview okně Tauri.
- `info` obsahuje `application` (language, platform `"windows"`, version), `plugin` (uuid, version), `devicePixelRatio`, `colors` a `devices`. Hlásíme **jedno virtuální zařízení** typu Stream Deck s velikostí podle rozložení aktivního profilu (radiální = 3×3, mřížka = cols×rows).
- Pokud plugin spadne, restartuj ho nejvýš 3× za minutu, pak ho označ jako chybný.

### 7.3 Události hostitel → plugin
`willAppear`, `willDisappear`, `keyDown`, `keyUp`, `didReceiveSettings`, `didReceiveGlobalSettings`, `deviceDidConnect`, `applicationDidLaunch`, `applicationDidTerminate` (pro `ApplicationsToMonitor`, využij modul `foreground`/`apps`), `systemDidWakeUp`, `propertyInspectorDidAppear`, `propertyInspectorDidDisappear`, `sendToPlugin`, `titleParametersDidChange`.

Každá instance tlačítka má unikátní `context` (stabilní ID, které se ukládá s tlačítkem). `payload` obsahuje `settings`, `coordinates` (slot převedený na column/row virtuálního zařízení), `state` a `isInMultiAction`.

**Viditelnost:** tlačítka aktuální stránky aktivního profilu jsou „appeared“ celou dobu, kdy je profil aktivní, **i když je overlay skrytý**. Pluginy tak můžou průběžně aktualizovat titulky a stavy (např. stav mikrofonu v Discordu). Při přepnutí profilu nebo stránky pošli `willDisappear` starým a `willAppear` novým tlačítkům.

Výběr tlačítka v panelu = `keyDown` a hned za ním `keyUp`.

### 7.4 Události plugin → hostitel
`setTitle`, `setImage` (data URL nebo SVG), `setState`, `showAlert`, `showOk`, `setSettings`, `getSettings`, `setGlobalSettings`, `getGlobalSettings`, `openUrl`, `logMessage`, `switchToProfile`, `sendToPropertyInspector`.

Nastavení se ukládá do `plugin-settings\<uuid>.json` a přežije restart.

### 7.5 Property Inspector
V editoru se po výběru tlačítka s plugin akcí vpravo zobrazí HTML Property Inspector pluginu v iframe nebo webview. Zavolej v něm `connectElgatoStreamDeckSocket(port, propertyInspectorUUID, "registerPropertyInspector", info, actionInfo)` podle dokumentace SDK a přeposílej zprávy mezi ním a pluginem.

### 7.6 Test bez cizích pluginů
Do `tests/fixtures/` napiš vlastní minimální testovací plugin v Node.js (`com.deckless.testplugin.sdPlugin`) s akcí „Counter“: při `keyDown` zvýší číslo a pošle `setTitle`, nastavení ukládá přes `setSettings`. Integrační test ověří celý cyklus: instalace → spuštění → registrace → willAppear → keyDown → setTitle → uložení nastavení.

---

## 8. Editor (okno nastavení)

Rozložení:
- **Vlevo:** seznam profilů (výchozí nahoře) a tlačítko „Přidat profil pro aplikaci“, které nabídne běžící procesy a nainstalované aplikace s ikonami.
- **Uprostřed:** živý náhled panelu (radiální nebo mřížka) ve stejném vzhledu jako overlay. Tlačítka jde přetahovat mezi sloty. Pravý klik nabídne duplikovat, smazat nebo vytvořit složku.
- **Vpravo:** výběr akcí podle kategorií (Klávesnice, Aplikace, Média a zvuk, Okna, Systém, OBS, Pluginy), s vyhledáváním. Akce se přetáhne do slotu. Po výběru tlačítka se tu ukáže formulář parametrů (pro plugin akce jeho Property Inspector).
- **Nahrávání zkratky:** pole „Stiskni zkratku“ zachytí kombinaci kláves, včetně Win.
- **Ikona tlačítka:** vestavěná sada (Lucide), vlastní obrázek (PNG/JPG/SVG/GIF, zmenšit na 144×144), emoji nebo ikona z exe.
- Tlačítko **„Vyzkoušet“** akci hned spustí.
- Změny se ukládají automaticky (debounce 500 ms), bez tlačítka Uložit.

Karta **Nastavení:** zkratka pro vyvolání, tlačítko myši, práh pro podržení, vzhled, jazyk, spuštění po přihlášení (ve výchozím stavu zapnuté), „Spouštět jako správce“ (vysvětlení viz 10.2; po zapnutí nabídni restart s elevací), připojení k OBS (host, port, heslo a tlačítko „Otestovat“), export a import všeho, otevření složky s logy.

Karta **Pluginy:** nainstalované pluginy, stav (běží / chyba / nekompatibilní), odinstalace, instalace ze souboru, odkaz na Elgato Marketplace s poznámkou, že plugin se stáhne tam a nainstaluje sem.

---

## 9. Jazyk

Všechny texty UI drž v `src/i18n/cs.json` a `src/i18n/en.json`. Výchozí je čeština, pokud je jazyk systému čeština nebo slovenština, jinak angličtina. Texty piš přirozeně a stručně, jako v běžné aplikaci, ne jako manuál.

---

## 10. Nefunkční požadavky

### 10.1 Výkon
- Zobrazení overlaye od stisku zkratky: **< 100 ms** (měř a zapiš do `PROGRESS.md`).
- Nečinná aplikace: CPU ~0 %, paměť cíleně pod 150 MB bez pluginů.
- Hooky nesmí přidat znatelnou latenci do psaní ani pohybu myši.

### 10.2 Známá omezení (uveď je v README a stručně v UI tam, kde dávají smysl)
- **Exkluzivní fullscreen ve hrách:** overlay se přes něj nevykreslí. Funguje režim „borderless windowed“ / „okno bez okrajů“.
- **Aplikace spuštěné jako správce:** Windows (UIPI) nedovolí neprivilegované aplikaci do nich posílat klávesy. Řešení je volba „Spouštět jako správce“. Když akce do takového okna selže, ukaž jednou nenásilnou nápovědu.
- **Anti-cheat** některých her může umělý vstup ignorovat nebo blokovat. Aplikace se ho nikdy nesmí snažit obejít.

### 10.3 Stabilita
- Jedna instance aplikace (`single-instance`). Druhé spuštění otevře editor.
- Panika v jakémkoli vlákně se zaloguje a nesmí shodit hooky. Pokud hook vlákno spadne, restartuj ho.
- Poškozený JSON profilu: přejmenuj ho na `.broken`, zaloguj a pokračuj s ostatními.

### 10.4 Testy
- Rust unit testy: výběr profilu podle exe, výpočet slotu z úhlu, serializace a migrace profilů, parsování zpráv SDK, sestavení sekvence `SendInput` z kombinace.
- Integrační test SDK hostitele s testovacím pluginem (7.6).
- Frontend: Vitest pro logiku editoru (přesouvání slotů, validace formulářů).
- `cargo clippy` bez varování, `tsc --noEmit` bez chyb, ESLint + Prettier.

---

## 11. Fáze a akceptační kritéria

### Fáze 0 – Základ
- Prostředí připravené (2.1), Tauri 2 + React + TS + Tailwind projekt, git repo, `CLAUDE.md`, `PROGRESS.md`, `DECISIONS.md`.
- Tray ikona, prázdné okno editoru, single-instance, logování.
- ✅ `npm run tauri dev` spustí aplikaci s tray ikonou, editor se otevře z tray.

### Fáze 1 – Overlay a základní akce
- Input hooky, overlay okno (radiální rozložení), podržení i ťuknutí, Esc a šipky, multi-monitor + DPI, návrat focusu (4.5).
- Akce `hotkey`, `typeText`, `launch`, `open`, `media`, `volume`, `micMute`, `system`.
- Výchozí profil z 5.1 uložený v JSON.
- ✅ Test s Poznámkovým blokem z 4.5 funguje.
- ✅ Napsaný text s diakritikou („Příliš žluťoučký kůň“) se vloží přesně.
- ✅ Panel se otevře u kurzoru na obou monitorech a nepřetéká přes okraj.
- ✅ Naměřená latence zobrazení < 100 ms.

### Fáze 2 – Editor
- Celý editor podle kapitoly 8 (bez karty Pluginy), rozložení mřížka, složky, ikony včetně extrakce z exe, import a export.
- ✅ Bez úpravy JSONu ručně: vytvořím tlačítko, změním ikonu, přetáhnu ho jinam, vytvořím složku a všechno to funguje v overlayi i po restartu.

### Fáze 3 – Profily aplikací a pokročilé akce
- Výběr profilu podle aplikace v popředí, doplňování z výchozího profilu, startovní profily (5.1), průvodce prvním spuštěním.
- Akce `appVolume`, `audioDevice`, `window`, `obs`, `multi`, `toggle`, `switchProfile`, `nowPlaying`.
- ✅ V prohlížeči se ukáže profil prohlížeče, na ploše výchozí.
- ✅ Přepnutí výstupu zvuku (např. reproduktory ↔ sluchátka) funguje a dá se ověřit v nastavení zvuku Windows.
- ✅ S OBS (WebSocket zapnutý) jde přepnout scénu a spustit nahrávání, stav nahrávání je vidět na tlačítku.

### Fáze 4 – Stream Deck pluginy
- Celá kapitola 7 včetně Property Inspectoru a karty Pluginy v editoru.
- ✅ Integrační test s testovacím pluginem prochází.
- ✅ Testovací plugin jde nainstalovat přetažením, akce „Counter“ v panelu počítá a hodnota přežije restart.
- Pokud je v prostředí dostupný nějaký veřejný open-source Stream Deck plugin pro Windows, vyzkoušej ho a výsledek zapiš do `PROGRESS.md`.

### Fáze 5 – Dokončení
- Instalátor NSIS (`npm run tauri build`), asociace `.streamDeckPlugin`, spuštění po přihlášení.
- Ikona aplikace (jednoduchá, vlastní, bez cizích značek).
- `README.md` česky: co to je, instalace, první kroky, jak nainstalovat plugin, známá omezení (10.2), jak sestavit ze zdrojáků.
- Projdi celý `SPEC.md` bod po bodu a do `PROGRESS.md` napiš, co je splněné a co ne, s důvodem.
- ✅ Instalátor se sestaví a aplikace z něj jde nainstalovat a spustit.

---

## 12. Co nedělat
- Žádné účty, cloud, telemetrie ani automatické odesílání čehokoli.
- Žádné obcházení anti-cheatů ani UAC.
- Žádný kód z projektů pod GPL (OpenDeck) ani z proprietárního software Elgato.
- Nepřidávej funkce mimo toto zadání, dokud nejsou hotové všechny fáze. Nápady zapisuj do `IDEAS.md`.

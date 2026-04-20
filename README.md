# kvaser_utils

Utility Linux per Kvaser Memorator Pro 2. L'interfaccia TUI Rust resta separata dai wrapper Kvaser: la vista non parla mai direttamente con CANlib, kvmlib o kvlclib.

## Stato SDK Kvaser

Ho verificato e scaricato i pacchetti ufficiali Kvaser:

- `linuxcan_5_51_461.tar.gz`: driver Linux e CANlib.
- `kvlibsdk_5_51_461.tar.gz`: kvmlib e kvlclib, necessari per leggere i log Memorator e convertirli in MF4.

Fonti ufficiali:

- CANlib WebHelp: https://kvaser.com/canlib-webhelp/
- Linux Drivers and SDK: `https://www.kvaser.com/downloads-kvaser/?utm_source=software&utm_ean=7330130980754&utm_status=latest`
- Linux SDK Library: `https://www.kvaser.com/downloads-kvaser/?utm_source=software&utm_ean=7330130981966&utm_status=latest`

Gli archivi e le sorgenti SDK sono ignorati da git. Per riscaricarli:

```bash
scripts/download-kvaser-sdk.sh
```

## Architettura

- `crates/kvaser-api`: wrapper Rust runtime-loaded per `libcanlib.so`, `libkvmlib.so`, `libkvlclib.so`. Nessun codice TUI.
- `crates/kvaser-core`: modelli e controller applicativi. Qui vive la logica MVC: CAN controller, log controller, SocketCAN Linux.
- `crates/kvaser-tui`: vista TUI e input handling con `ratatui`/`crossterm`.

## Funzioni iniziali

- Elenco canali Kvaser visti da CANlib.
- Attivazione canali CAN 0 e 1 con bitrate indipendenti, mantenendo aperti gli handle mentre la TUI resta attiva.
- Cambio bitrate dalla TUI tra 500k e 1M.
- Switch del Memorator tra `interface` e `logger` mode via CANlib. `logger` mode permette al device di registrare anche mentre e' collegato via USB, se la configurazione logger sul device e' valida.
- Monitor live dei frame CAN in TUI dai canali accesi in `interface` mode.
- Configurazione Linux SocketCAN (`can0`, `can1`, o nomi custom) con bitrate indipendenti.
- Elenco log nel Memorator via kvmlib.
- Preview dei primi eventi di un log.
- Export MF4 di tutti i log selezionati via kvlclib (`KVLC_FILE_FORMAT_MDF_4X`), leggendo direttamente dal device.
- Reset/formattazione della SD del Memorator via `kvmDeviceFormatDisk`.

## Prerequisiti Linux

Installa i pacchetti Kvaser seguendo i README dentro `third_party/kvaser/linuxcan` e `third_party/kvaser/kvlibsdk`. A runtime devono essere raggiungibili:

- `libcanlib.so`
- `libkvmlib.so`
- `libkvlclib.so`

Se le librerie non sono installate in un path standard:

```bash
export LD_LIBRARY_PATH=/path/to/kvaser/libs:$LD_LIBRARY_PATH
```

SocketCAN richiede il comando `ip` e permessi sufficienti per modificare le interfacce CAN.

## Runtime locale senza installazione in `/usr`

Per sviluppo puoi buildare le librerie user-space negli SDK scaricati:

```bash
scripts/build-kvaser-local.sh
```

Poi usa il wrapper che imposta `LD_LIBRARY_PATH`:

```bash
scripts/run-local.sh channels
scripts/run-local.sh tui
scripts/run-local.sh logs list --device-channel 0
```

Questo risolve il caricamento di `libcanlib.so`, `libkvmlib.so` e `libkvlclib.so`. Per vedere davvero il Memorator come hardware CAN serve comunque il driver Linux Kvaser installato/caricato a livello sistema, per esempio dai target `sudo make install` o DKMS dello SDK `linuxcan`.

## LinuxCAN Kvaser + SocketCAN per altri device

Il Memorator Pro 2xHS v2 viene preso dal driver SocketCAN del kernel `kvaser_usb` se questo e' disponibile. In quel caso `ip link` mostra `can0` e `can1` con driver `kvaser_usb_kcan`, ma `kvmlib`/CANlib Kvaser non riescono a comunicare con il logger e possono fallire con `kvmDeviceOpen` status `-37`.

Per usare LinuxCAN sul Kvaser e continuare a usare SocketCAN per gli altri adattatori:

```bash
scripts/build-kvaser-linuxcan-modules.sh
scripts/install-kvaser-linuxcan-mhydra.sh
```

Lo script installa solo i moduli Kvaser necessari al Memorator Pro 2:

- `kvcommon`
- `mhydra`

Lo script Kvaser blacklista solo `kvaser_usb`. Non rimuove `can`, `can_raw`, `can_dev`, `gs_usb`, `peak_usb` o altri driver SocketCAN non Kvaser.

Limite importante: lo stesso hardware Kvaser non puo' essere controllato contemporaneamente da `kvaser_usb` SocketCAN e da LinuxCAN `mhydra`. Per il Memorator usiamo `mhydra`; per gli altri device CAN continui a usare SocketCAN.

## Uso

TUI:

```bash
cargo run -p kvaser-tui -- tui
```

Comandi utili:

```bash
cargo run -p kvaser-tui -- channels
cargo run -p kvaser-tui -- logs list --device-channel 0
cargo run -p kvaser-tui -- logs preview --device-channel 0 --index 0 --limit 20
cargo run -p kvaser-tui -- logs export --device-channel 0 --output exports/session.mf4 --logs 0,1,2
cargo run -p kvaser-tui -- socketcan up --if0 can0 --bitrate0 500k --if1 can1 --bitrate1 250k
```

Se una SD nuova risulta `kvmERR_FILE_SYSTEM_CORRUPT` (`-28`) o non formattata, puoi reinizializzarla dal logger. Il comando e' distruttivo e richiede `--yes`:

```bash
scripts/run-local.sh logs reset --device-channel 0 --filesystem fat32 --yes
```

Opzioni utili: `--reserve-mb` riserva spazio per file utente, `--database-mb` riserva spazio per database. I default sono `0`.

Tasti TUI:

- `1`: canali CAN.
- `2`: log Memorator.
- `3`: frame CAN live.
- `r`: refresh.
- `q`: esci.
- `j/k` o frecce: muovi selezione.
- Globale: `s` mette il Memorator in `logger` mode USB, `i` torna in `interface` mode.
- CAN: `b` cambia bitrate del canale selezionato tra 500k e 1M, `a` attiva CAN 0 e CAN 1, `o` attiva il selezionato, `f` spegne il selezionato.
- Log: `spazio` seleziona/deseleziona, `A` seleziona tutti, `c` pulisce selezione, `p` preview, `e` export MF4 dei selezionati.
- Live: `c` pulisce i frame visualizzati. I frame arrivano solo dai canali accesi in `interface` mode.

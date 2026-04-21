# kvaser_utils

Utility Linux per Kvaser Memorator Pro 2 con:
- TUI live/logs
- wrapper CANlib/kvmlib/kvlclib
- tool CLI
- bridge live bidirezionale LinuxCAN <-> SocketCAN

## 1) Installazione

### 1.1 Prerequisiti

Servono:
- Rust toolchain (`cargo`)
- `ip` (`iproute2`)
- `sudo`
- Linux (progetto Linux-only)

### 1.2 SDK Kvaser nel repository

Questo repository contiene gia' gli SDK Kvaser sotto `third_party/kvaser`.
Se vuoi riscaricarli dalla fonte ufficiale:

```bash
scripts/download-kvaser-sdk.sh
```

### 1.3 Build librerie user-space locali

Compila CANlib/kvmlib/kvlclib locali:

```bash
scripts/build-kvaser-local.sh
```

Per eseguire il progetto con `LD_LIBRARY_PATH` gia' configurato:

```bash
scripts/run-local.sh channels
scripts/run-local.sh tui
```

In alternativa puoi caricare le env manualmente:

```bash
source scripts/kvaser-env.sh
cargo run -p kvaser-tui -- channels
```

## 2) Modalita' driver Kvaser: SocketCAN o LinuxCAN

Hai uno switch unico (senza rebuild):

```bash
scripts/kvaser-driver-mode.sh socketcan
scripts/kvaser-driver-mode.sh linuxcan
scripts/kvaser-driver-mode.sh status
```

Output atteso:
- `KVASER -> SocketCAN (kvaser_usb)`
- `KVASER -> LinuxCAN (mhydra)`

### 2.1 Modalita' LinuxCAN (mhydra)

Per usare il Kvaser via LinuxCAN:

```bash
scripts/kvaser-driver-mode.sh linuxcan
```

Se devi installare i moduli LinuxCAN la prima volta:

```bash
scripts/build-kvaser-linuxcan-modules.sh
scripts/install-kvaser-linuxcan-mhydra.sh
```

### 2.2 Modalita' SocketCAN (kvaser_usb)

Per usare il Kvaser come interfaccia SocketCAN standard:

```bash
scripts/kvaser-driver-mode.sh socketcan
```

Script legacy equivalente:

```bash
scripts/disable-kvaser-linuxcan-enable-socketcan.sh
```

### 2.3 Interfacce `can0/can1` per tunnel live

Per creare rapidamente `can0` e `can1` virtuali (`vcan`) per i tool SocketCAN:

```bash
scripts/setup-socketcan-tunnel-ifaces.sh
```

## 3) Avvio TUI

Con env locale gia' impostate dal wrapper:

```bash
scripts/run-local.sh tui
```

Oppure direttamente con cargo:

```bash
cargo run -p kvaser-tui -- tui
```

Tasti principali TUI:
- `1`: tab CAN
- `2`: tab Logs
- `3`: tab Live
- `r`: refresh
- `q`: quit
- `j/k` o frecce: navigazione
- Globale: `s` logger mode USB, `i` interface mode
- CAN: `b` bitrate, `a` on CAN0+CAN1, `o` on selected, `f` off selected
- Logs: `space` select, `A` select all, `c` clear, `p` preview, `e` export
- Live: `c` clear frames

Note live tunnel:
- Se `can0/can1` esistono, in Live il tunnel e' bidirezionale:
  LinuxCAN -> SocketCAN e SocketCAN -> LinuxCAN sui canali 0/1.

## 4) Tutti gli script disponibili

- `scripts/download-kvaser-sdk.sh`: scarica archivi SDK Kvaser ufficiali
- `scripts/build-kvaser-local.sh`: build librerie user-space locali
- `scripts/kvaser-env.sh`: esporta `LD_LIBRARY_PATH` locale
- `scripts/run-local.sh`: esegue `kvaser-tui` con env locale
- `scripts/build-kvaser-linuxcan-modules.sh`: build moduli kernel LinuxCAN (`kvcommon`, `mhydra`)
- `scripts/install-kvaser-linuxcan-mhydra.sh`: installa/attiva LinuxCAN per Kvaser
- `scripts/setup-socketcan-tunnel-ifaces.sh`: crea/porta UP `can0`/`can1` (`vcan`)
- `scripts/disable-kvaser-linuxcan-enable-socketcan.sh`: disattiva LinuxCAN e riattiva `kvaser_usb`
- `scripts/kvaser-driver-mode.sh`: switch rapido `linuxcan` / `socketcan` / `status`

## 5) Tutti i comandi CLI

Binario:

```bash
cargo run -p kvaser-tui -- <comando>
```

### 5.1 Generali

```bash
cargo run -p kvaser-tui -- tui [--device-channel 0]
cargo run -p kvaser-tui -- channels
cargo run -p kvaser-tui -- mode --channel 0 interface
cargo run -p kvaser-tui -- mode --channel 0 logger
```

### 5.2 SocketCAN helper

```bash
cargo run -p kvaser-tui -- socketcan up --if0 can0 --bitrate0 500k --if1 can1 --bitrate1 250k
cargo run -p kvaser-tui -- socketcan down --iface can0
```

### 5.3 Logs

```bash
cargo run -p kvaser-tui -- logs list --device-channel 0
cargo run -p kvaser-tui -- logs preview --device-channel 0 --index 0 --limit 20
cargo run -p kvaser-tui -- logs export --device-channel 0 --output exports/session.mf4 --logs 0,1,2 --overwrite true
cargo run -p kvaser-tui -- logs reset --device-channel 0 --filesystem fat32 --reserve-mb 0 --database-mb 0 --yes
```

`logs record-usb` (configura logger e registra da USB):

```bash
cargo run -p kvaser-tui -- logs record-usb --device-channel 0 --can-channel 0 --bitrate 500k --seconds 10
cargo run -p kvaser-tui -- logs record-usb --device-channel 0 --both --bitrate 500k --seconds 10
cargo run -p kvaser-tui -- logs record-usb --device-channel 0 --both --bitrate 500k --seconds 10 --leave-logger
```

### 5.4 DBC

```bash
cargo run -p kvaser-tui -- logs dbc list
cargo run -p kvaser-tui -- logs dbc add --channel 0 --file /path/file.dbc
cargo run -p kvaser-tui -- logs dbc remove --file /path/file.dbc
cargo run -p kvaser-tui -- logs dbc clear-channel --channel 0
cargo run -p kvaser-tui -- logs dbc clear
```

## 6) Troubleshooting rapido

- Se cambi driver e non vedi subito i canali: scollega/ricollega il Kvaser.
- Verifica modalita' corrente:

```bash
scripts/kvaser-driver-mode.sh status
```

- Se usi TUI live con tunnel e vuoi tool SocketCAN locali (`candump`, `cansend`), assicurati che `can0/can1` esistano (`setup-socketcan-tunnel-ifaces.sh`).

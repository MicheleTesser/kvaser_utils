# kvaser_utils

Utility Linux per Kvaser Memorator Pro 2 con:
- TUI live/logs
- wrapper CANlib/kvmlib/kvlclib
- tool CLI
- bridge live bidirezionale LinuxCAN <-> SocketCAN

## 1) Installazione

Su Arch Linux, CachyOS e derivate x86-64 il percorso consigliato e' quello dei
[pacchetti pacman con DKMS](#41-pacchetti-pacman-distribuibili-arch-linux-e-derivate-x86-64).
Chi riceve i pacchetti puo' installarli senza clonare il repository o compilare
l'applicazione Rust. DKMS si occupa invece di compilare i driver sul PC destinatario.

Le sezioni 1.1–1.3 descrivono l'esecuzione dai sorgenti.

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

Il binario release prova anche automaticamente questi percorsi libreria, quindi
puoi avviarlo senza wrapper se le `.so` Kvaser sono li':
- `./third_party/kvaser/...` dal root del repository
- `/usr/lib/kvaser_utils`
- `/usr/local/lib/kvaser_utils`
- `../lib/kvaser_utils` relativo all'eseguibile

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

Con i pacchetti pacman i moduli vengono gestiti da DKMS: usare `kvaser_mode -l`.
I comandi seguenti sono solo per l'installazione manuale, alternativa al pacchetto,
e non garantiscono la ricompilazione dopo gli aggiornamenti del kernel:

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

Con il pacchetto pacman installato:

```bash
kvaser-tui tui
```

Con env locale gia' impostate dal wrapper:

```bash
scripts/run-local.sh tui
```

Oppure direttamente con cargo:

```bash
cargo run -p kvaser-tui -- tui
```

Build release diretta:

```bash
cargo build --release -p kvaser-tui
./target/release/kvaser-tui tui
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

- `scripts/build-packages.sh`: genera i due pacchetti pacman in `dist/`
- `scripts/create-release-archive.sh`: genera l'archivio sorgente distribuibile
- `scripts/download-kvaser-sdk.sh`: scarica archivi SDK Kvaser ufficiali
- `scripts/build-kvaser-local.sh`: build librerie user-space locali
- `scripts/kvaser-env.sh`: esporta `LD_LIBRARY_PATH` locale
- `scripts/run-local.sh`: esegue `kvaser-tui` con env locale
- `scripts/build-kvaser-linuxcan-modules.sh`: build moduli kernel LinuxCAN (`kvcommon`, `mhydra`)
- `scripts/install-kvaser-linuxcan-mhydra.sh`: installa/attiva LinuxCAN per Kvaser
- `scripts/setup-socketcan-tunnel-ifaces.sh`: crea/porta UP `can0`/`can1` (`vcan`)
- `scripts/disable-kvaser-linuxcan-enable-socketcan.sh`: disattiva LinuxCAN e riattiva `kvaser_usb`
- `scripts/kvaser-driver-mode.sh`: switch rapido `linuxcan` / `socketcan` / `status`
- `scripts/kvaser_mode`: wrapper corto per packaging/installazione (`-l`, `-s`, `-t`)

## 4.1 Pacchetti pacman distribuibili (Arch Linux e derivate x86-64)

Per creare i pacchetti, con `base-devel`, Rust, Clang e libxml2 installati:

```bash
scripts/build-packages.sh -s
```

Il comando genera un archivio sorgente, ne fissa il checksum e compila in `dist/`.
Non installa i pacchetti e non modifica i driver in uso. I file da mandare sono:

- `dist/kvaser-utils-0.1.0-2-x86_64.pkg.tar.zst`: TUI, comandi e librerie Kvaser.
- `dist/kvaser-linuxcan-dkms-0.1.0-2-x86_64.pkg.tar.zst`: sorgenti dei moduli
  `kvcommon` e `mhydra`, patch di compatibilita' e configurazione DKMS.

Inviare **entrambi i file `.pkg.tar.zst` della stessa versione**. Non occorre
mandare `dist/src/`, `dist/pkg/`, la cartella `target/` o gli SDK separatamente.
L'archivio `.tar.gz` e' una release sorgente, non un pacchetto da dare a `pacman -U`.

Sul PC destinatario, aprire un terminale nella cartella contenente i due file.
Installare gli header corrispondenti al kernel, poi entrambi
i pacchetti nella stessa transazione. Esempio per il kernel Arch `linux`:

```bash
sudo pacman -Syu linux-headers
sudo pacman -U ./kvaser-utils-0.1.0-2-x86_64.pkg.tar.zst ./kvaser-linuxcan-dkms-0.1.0-2-x86_64.pkg.tar.zst
dkms status
kvaser_mode -l
kvaser-tui channels
kvaser-tui tui
```

Per `linux-lts` usare `linux-lts-headers`; per CachyOS usare gli header della
variante installata, ad esempio `linux-cachyos-headers`. Con piu' kernel servono
gli header di ciascuno. Le altre dipendenze (DKMS e compilatori inclusi) vengono
risolte da pacman dai repository configurati: per la prima installazione serve
accesso ai repository o una cache completa. Non servono Rust e il repository
sorgente sul PC destinatario.

DKMS ricompila i driver automaticamente dopo gli aggiornamenti del kernel e
degli header tramite gli hook pacman. La compilazione usa la configurazione
del kernel di destinazione, anche se non e' quello attualmente avviato, e
seleziona LLVM per i kernel compilati con Clang. Prima di riavviare controllare
che `dkms status` mostri `installed` per il nuovo kernel e che pacman non abbia
segnalato errori. Per riprovare dopo aver installato header mancanti:

```bash
sudo dkms autoinstall
```

Per aggiornare successivamente l'applicazione e i driver, ricevere o generare
la nuova coppia di pacchetti e installarla nuovamente con `sudo pacman -U`
indicando entrambi i file. La sola distribuzione dei file non configura un
repository pacman: `pacman -Syu` non scarica nuove versioni di kvaser-utils.
Gli aggiornamenti del kernel continuano invece ad attivare la ricompilazione DKMS.

Il default e' LinuxCAN. `kvaser_mode -s` seleziona SocketCAN e salva la scelta
per i riavvii; `kvaser_mode -l` ripristina LinuxCAN. L'installazione del pacchetto
non forza il cambio del driver gia' caricato: usare il comando e, se necessario,
scollegare e ricollegare il dispositivo.

Nessun driver esterno puo' garantire compatibilita' con ogni kernel futuro:
modifiche alle API possono richiedere una nuova patch e un aggiornamento del
pacchetto. Con Secure Boot che impone firme, occorre configurare una chiave
DKMS accettata dal sistema. I binari sono per Linux x86-64 con le dipendenze
indicate da pacman; aggiornamenti incompatibili delle librerie di sistema
possono richiedere una ricompilazione. Sono escluse le ottimizzazioni CPU native.

### Comandi disponibili dopo l'installazione

| Comando | Funzione |
| --- | --- |
| `kvaser-tui tui` | Apre la TUI |
| `kvaser-tui channels` | Elenca i canali rilevati tramite CANlib |
| `kvaser_mode -l` | Attiva LinuxCAN e salva la scelta per i riavvii |
| `kvaser_mode -s` | Attiva SocketCAN e salva la scelta per i riavvii |
| `kvaser_mode -t` | Mostra lo stato dei driver |
| `kvaser_socketcan_ifaces` | Prepara le interfacce virtuali per il tunnel |
| `dkms status` | Mostra lo stato dei moduli per ciascun kernel |

I comandi che cambiano driver o configurano le interfacce richiedono privilegi
amministrativi e usano `sudo` quando necessario.

### Verifiche eseguite sui pacchetti 0.1.0-2

Durante la preparazione dei pacchetti sono stati verificati:

- build completa dei due pacchetti con `makepkg`;
- controllo con `namcap`, senza errori bloccanti; restano avvisi, fra cui
  l'assenza di FULL RELRO in `libcanlib`;
- avvio del binario estratto dal pacchetto, fuori dal repository, con
  `--help`, `channels` e `logs dbc list`;
- compilazione di `kvcommon` e `mhydra` per `7.2.11-1-cachyos` con Clang
  e `6.18.58-rt9-arch1-1-rt-lts` con GCC;
- compilazione per CachyOS anche a partire dai sorgenti estratti dal pacchetto DKMS.

Queste verifiche non includono un'installazione sul sistema, l'esecuzione degli
hook DKMS durante un vero aggiornamento o una prova di traffico CAN sul dispositivo.
La compilazione riuscita sui kernel elencati non certifica ogni versione futura.

### Migrazione dall'installazione manuale

Non eseguire piu' `install-kvaser-linuxcan-mhydra.sh` dopo il passaggio a DKMS.
Il vecchio installer puo' aver lasciato `kvcommon.ko` e `mhydra.ko` sotto
`/usr/lib/modules/<kernel>/kernel/drivers/usb/misc/` e il file
`/etc/modprobe.d/blacklist-kvaser_usb.conf`. Controllare la provenienza con
`pacman -Qo <file>` prima di rimuovere eventuali copie manuali; non eliminare
file appartenenti ad altri pacchetti. Dopo la pulizia eseguire
`sudo depmod -a` e `sudo dkms autoinstall`, quindi riavviare. La vecchia blacklist
va rimossa se si vuole usare SocketCAN come default. Non vengono eliminati
automaticamente file preesistenti durante l'installazione.

## 4.2 Creazione archivio release sorgente

```bash
scripts/create-release-archive.sh
```

Genera `dist/kvaser-utils-0.1.0.tar.gz`, con codice, SDK, patch, packaging e
script di build. Chi lo riceve puo' estrarlo ed eseguire
`scripts/build-packages.sh -s` per ricreare entrambi i pacchetti.
Per ripetere una build gia' completata usare `scripts/build-packages.sh -sf`.

Riferimenti: [DKMS su Arch](https://wiki.archlinux.org/title/Dynamic_Kernel_Module_Support)
e [linee guida pacchetti DKMS](https://wiki.archlinux.org/title/DKMS_package_guidelines).

## 5) Tutti i comandi CLI

Con il pacchetto installato, tutti gli esempi seguenti si eseguono sostituendo
`cargo run -p kvaser-tui --` con `kvaser-tui`. Ad esempio:

```bash
kvaser-tui logs list --device-channel 0
```

Dai sorgenti:

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

### Diagnostica dell'installazione pacman/DKMS

```bash
uname -r
dkms status
modinfo -n mhydra
modinfo -F vermagic mhydra
kvaser_mode -t
kvaser-tui channels
```

- **Modulo assente dopo un aggiornamento:** verificare che siano installati gli
  header della stessa versione del kernel, poi eseguire `sudo dkms autoinstall`.
  Prima del riavvio, `uname -r` mostra ancora il vecchio kernel: controllare anche
  la voce del nuovo kernel in `dkms status`.
- **Compilazione DKMS fallita:** leggere il log sotto
  `/var/lib/dkms/kvaser-linuxcan/<versione>/`, normalmente `build/make.log`.
  Per questi pacchetti la versione DKMS e' `0.1.0-2`. Correggere header o toolchain
  mancanti; se l'errore riguarda API kernel incompatibili serve una patch aggiornata.
- **Modulo compilato ma non caricabile:** controllare i messaggi del kernel con
  `sudo journalctl -k -b`. Errori di firma richiedono la configurazione delle
  chiavi DKMS/Secure Boot; errori di versione possono indicare vecchi moduli manuali.
- **SocketCAN non persiste al riavvio:** controllare l'eventuale blacklist lasciata
  dal vecchio installer, come descritto nella sezione di migrazione.
- **Libreria condivisa mancante o dipendenza non soddisfatta:** verificare le
  dipendenze indicate da `pacman -U`. Non forzare l'installazione ignorandole;
  dopo cambiamenti incompatibili delle librerie ricreare i pacchetti con
  `scripts/build-packages.sh -sf` su un sistema aggiornato.

Il README viene installato anche in `/usr/share/doc/kvaser-utils/README.md`.

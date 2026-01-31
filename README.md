# network-watchdog

Network Watchdog: automatically recover connectivity by turning on Wi‑Fi and connecting to saved profiles when the network is down (Windows).

## What it does

- Periodically checks network reachability (NCSI).
- If unreachable: turns on Wi‑Fi radio, enables WLAN adapter if needed, then tries saved Wi‑Fi profiles (filtered by visibility or by your options) until the network is restored or all attempts fail.
- Runs in a loop by default, or once with `--once`.

## Requirements

- **Windows** (uses WLAN API, NCSI, PowerShell/netsh for adapter).
- **Administrator** rights (the program will try to elevate via [gsudo](https://github.com/gerardog/gsudo) if not already admin).

## Build

```bash
cargo build --release
```

## Usage

```text
network-watchdog [OPTIONS]
```

| Option                     | Description                                                                                                                          |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `--once`, `-1`, `--single` | Run once: one network check and one recovery attempt, then exit.                                                                     |
| `--interval <SECS>`        | Check interval in seconds (default: 60).                                                                                             |
| `--ncsi-url <URL>`         | NCSI probe URL (default: `http://www.msftconnecttest.com/connecttest.txt`).                                                          |
| `--ncsi-timeout <SECS>`    | NCSI request timeout in seconds (default: 5).                                                                                        |
| `--all`                    | Try all saved Wi‑Fi profiles (no “visible only” filter). Default is scan-only (only profiles that match currently visible networks). |
| `--profiles <NAME>...`     | Only try these saved profile names, e.g. `--profiles Home --profiles Office` or `--profiles "Home,Office"`.                          |

### Examples

- Run in background, check every 60 seconds (default), recover using visible-only profiles:
  ```bash
  network-watchdog
  ```
- Run once and exit (e.g. for a scheduled task):
  ```bash
  network-watchdog --once
  ```
- Check every 30 seconds:
  ```bash
  network-watchdog --interval 30
  ```
- Custom NCSI URL and timeout:
  ```bash
  network-watchdog --ncsi-url "http://example.com/" --ncsi-timeout 10
  ```
- Run once and try only profiles named “Home” or “Office”:
  ```bash
  network-watchdog --once --profiles Home,Office
  ```
- Try every saved profile (not only visible):
  ```bash
  network-watchdog --all
  ```

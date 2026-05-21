# ping-parser

Reads the **Ping Statistics** section from a performance-report text file and
appends a row to `PR Consolidated.ods` (LibreOffice Calc / OpenDocument Spreadsheet).

---

## Build

```bash
# Requires Rust ≥ 1.70  (https://rustup.rs)
cargo build --release
# Binary: target/release/ping_parser
```

---

## Usage

```bash
# Default output → "PR Consolidated.ods" in the current directory
ping_parser pr-202605211200.txt

# Custom output path
ping_parser pr-202605211200.txt /path/to/PR\ Consolidated.ods
```

Each run **appends one row** to the sheet *Ping Reports*.  
If the ODS file does not exist it is created with a header row automatically.

---

## Supported input formats

### Linux / macOS `ping`

```
PING 8.8.8.8 (8.8.8.8) 56(84) bytes of data.
...

--- 8.8.8.8 ping statistics ---
10 packets transmitted, 9 received, 10% packet loss, time 9013ms
rtt min/avg/max/mdev = 12.345/15.678/45.123/2.345 ms
```

### Windows `ping`

```
Ping statistics for 8.8.8.8:
    Packets: Sent = 4, Received = 4, Lost = 0 (0% loss),
Approximate round trip times in milli-seconds:
    Minimum = 12ms, Maximum = 45ms, Average = 15ms
```

> Windows ping does not report `mdev`; that column is written as `0`.

---

## Output columns (sheet: *Ping Reports*)

| Column | Description |
|---|---|
| Report Date/Time | Parsed from filename `pr-YYYYMMDDHHMI.txt` |
| Host | Destination IP or hostname |
| Pkts Sent | Packets transmitted |
| Pkts Recv | Packets received |
| Loss % | Packet loss percentage |
| RTT Min (ms) | Round-trip time minimum |
| RTT Avg (ms) | Round-trip time average |
| RTT Max (ms) | Round-trip time maximum |
| RTT Mdev (ms) | Round-trip time mean deviation (Linux only) |

---

## Batch processing

```bash
# Process all txt files in a directory
for f in reports/pr-*.txt; do
    ping_parser "$f" "PR Consolidated.ods"
done
```

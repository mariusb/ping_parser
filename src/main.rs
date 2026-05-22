use chrono::NaiveDateTime;
use regex::Regex;
use spreadsheet_ods::write_ods;
use std::env;
use std::fs;
use std::path::Path;

// ── Parsed ping report data ─────────────────────────────────────────────────

#[derive(Debug)]
struct PingStats {
    /// Date/time parsed from the filename (pr-YYYYMMDDHHMI.txt)
    report_dt: String,
    /// Destination host or IP
    host: String,
    /// Packets transmitted
    transmitted: u32,
    /// Packets received
    received: u32,
    /// Packets duplicated
    _duplicates: u32,
    /// Packet loss percentage (e.g. 2.0)
    loss_pct: f64,
    /// Round-trip time min/avg/max/mdev in ms
    rtt_min: f64,
    rtt_avg: f64,
    rtt_max: f64,
    rtt_mdev: f64,
}

// ── Parse the txt file ───────────────────────────────────────────────────────

fn parse_ping_file(path: &Path) -> Result<PingStats, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file {}: {e}", path.display()))?;

    // ── Extract date/time from filename: pr-YYYYMMDDHHMI.txt ────────────────
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    let dt_re = Regex::new(r"pr-(\d{12})").unwrap();
    let report_dt = if let Some(cap) = dt_re.captures(stem) {
        let raw = &cap[1]; // "202605211200"
        match NaiveDateTime::parse_from_str(raw, "%Y%m%d%H%M") {
            Ok(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
            Err(_) => raw.to_string(),
        }
    } else {
        stem.to_string()
    };

    // ── Find the "ping statistics" section ──────────────────────────────────
    // Supports both Linux and Windows ping output.
    //
    // Linux example:
    //   --- 8.8.8.8 ping statistics ---
    //   10 packets transmitted, 9 received, 10% packet loss, time 9013ms
    //   rtt min/avg/max/mdev = 12.345/15.678/45.123/2.345 ms
    //
    // Windows example:
    //   Ping statistics for 8.8.8.8:
    //       Packets: Sent = 4, Received = 4, Lost = 0 (0% loss),
    //   Approximate round trip times in milli-seconds:
    //       Minimum = 12ms, Maximum = 45ms, Average = 15ms

    // ── Linux-style host header ──────────────────────────────────────────────
    let host_re = Regex::new(r"---\s+(.+?)\s+ping statistics\s+---").unwrap();
    // Windows-style host header
    let host_win_re = Regex::new(r"Ping statistics for\s+(.+?):").unwrap();

    let host = if let Some(cap) = host_re.captures(&content) {
        cap[1].trim().to_string()
    } else if let Some(cap) = host_win_re.captures(&content) {
        cap[1].trim().to_string()
    } else {
        return Err("Could not find ping statistics section in file".into());
    };

    // ── Packet counts ────────────────────────────────────────────────────────
    // Linux: "10 packets transmitted, 9 received, 10% packet loss"
    // MacOs: "500 packets transmitted, 495 packets received, 1.0% packet loss"
    let pkt_linux = Regex::new(
        r"(\d+)\s+packets transmitted,\s*(\d+)\s+packets received,(?:\s+\+(\d+) duplicates,)? \s*([\d.]+)%\s+packet loss",
    )
    .unwrap();
    // Windows: "Packets: Sent = 4, Received = 4, Lost = 0 (0% loss)"
    let pkt_win = Regex::new(
        r"Sent\s*=\s*(\d+),\s*Received\s*=\s*(\d+),\s*Lost\s*=\s*\d+\s*\(([\d.]+)%\s*loss\)",
    )
    .unwrap();

    let (transmitted, received, _duplicates, loss_pct) =
        if let Some(cap) = pkt_linux.captures(&content) {
            (
                cap[1].parse::<u32>().unwrap_or(0),
                cap[2].parse::<u32>().unwrap_or(0),
                cap.get(3)
                    .map_or(0, |m| m.as_str().parse::<u32>().unwrap_or(0)),
                cap[4].parse::<f64>().unwrap_or(0.0),
            )
        } else if let Some(cap) = pkt_win.captures(&content) {
            let tx: u32 = cap[1].parse().unwrap_or(0);
            let rx: u32 = cap[2].parse().unwrap_or(0);
            let dup: u32 = 0;
            let loss: f64 = cap[3].parse().unwrap_or(0.0);
            (tx, rx, dup, loss)
        } else {
            return Err("Could not parse packet counts from ping statistics".into());
        };

    // ── RTT ──────────────────────────────────────────────────────────────────
    // Linux: "rtt min/avg/max/mdev = 12.345/15.678/45.123/2.345 ms"
    // MacOs: "round-trip min/avg/max/stddev = 30.291/62.606/216.344/35.042 ms"
    let rtt_linux =
        Regex::new(r"round-trip min/avg/max/stddev\s*=\s*([\d.]+)/([\d.]+)/([\d.]+)/([\d.]+)\s*ms")
            .unwrap();
    // Windows: "Minimum = 12ms, Maximum = 45ms, Average = 15ms"
    let rtt_win = Regex::new(
        r"Minimum\s*=\s*([\d.]+)ms,\s*Maximum\s*=\s*([\d.]+)ms,\s*Average\s*=\s*([\d.]+)ms",
    )
    .unwrap();

    let (rtt_min, rtt_avg, rtt_max, rtt_mdev) = if let Some(cap) = rtt_linux.captures(&content) {
        (
            cap[1].parse::<f64>().unwrap_or(0.0),
            cap[2].parse::<f64>().unwrap_or(0.0),
            cap[3].parse::<f64>().unwrap_or(0.0),
            cap[4].parse::<f64>().unwrap_or(0.0),
        )
    } else if let Some(cap) = rtt_win.captures(&content) {
        let min: f64 = cap[1].parse().unwrap_or(0.0);
        let max: f64 = cap[2].parse().unwrap_or(0.0);
        let avg: f64 = cap[3].parse().unwrap_or(0.0);
        (min, avg, max, 0.0) // Windows doesn't report mdev
    } else {
        return Err("Could not parse RTT values from ping statistics".into());
    };

    Ok(PingStats {
        report_dt,
        host,
        transmitted,
        received,
        _duplicates,
        loss_pct,
        rtt_min,
        rtt_avg,
        rtt_max,
        rtt_mdev,
    })
}

// ── Column definitions ───────────────────────────────────────────────────────

const HEADERS: &[&str] = &[
    "Report Date/Time",
    "Host",
    "Pkts Sent",
    "Pkts Recv",
    "Loss %",
    "RTT Min (ms)",
    "RTT Avg (ms)",
    "RTT Max (ms)",
    "RTT Mdev (ms)",
];

// ── Write / append to the ODS file ──────────────────────────────────────────

fn append_to_ods(ods_path: &Path, stats: &PingStats) -> Result<(), String> {
    use spreadsheet_ods::{read_ods, Sheet, Value, WorkBook};

    // Load existing workbook or create a fresh one.
    let mut wb: WorkBook = if ods_path.exists() {
        read_ods(ods_path).map_err(|e| format!("Cannot read ODS: {e}"))?
    } else {
        WorkBook::new_empty()
    };

    // ── Ensure our sheet exists ──────────────────────────────────────────────
    const SHEET_NAME: &str = "Ping Reports";

    let sheet_idx = (0..wb.num_sheets()).find(|&i| wb.sheet(i).name() == SHEET_NAME);

    if sheet_idx.is_none() {
        let mut sheet = Sheet::new(SHEET_NAME);

        // Header row (row 0)
        for (col, &header) in HEADERS.iter().enumerate() {
            sheet.set_value(0, col as u32, header);
        }
        wb.push_sheet(sheet);
    }

    let sheet_idx = (0..wb.num_sheets())
        .find(|&i| wb.sheet(i).name() == SHEET_NAME)
        .unwrap();

    // Find the first empty row after existing data.
    let next_row = {
        let sheet = wb.sheet(sheet_idx);
        let mut row = 1u32;
        loop {
            // If column 0 (Date) is empty this row is free.
            if sheet.value(row, 0) == &Value::Empty {
                break row;
            }
            row += 1;
        }
    };

    // Write the new data row.
    {
        let sheet = wb.sheet_mut(sheet_idx);
        sheet.set_value(next_row, 0, stats.report_dt.as_str());
        sheet.set_value(next_row, 1, stats.host.as_str());
        sheet.set_value(next_row, 2, stats.transmitted as i32);
        sheet.set_value(next_row, 3, stats.received as i32);
        sheet.set_value(next_row, 4, stats.loss_pct);
        sheet.set_value(next_row, 5, stats.rtt_min);
        sheet.set_value(next_row, 6, stats.rtt_avg);
        sheet.set_value(next_row, 7, stats.rtt_max);
        sheet.set_value(next_row, 8, stats.rtt_mdev);
    }

    write_ods(&mut wb, ods_path).map_err(|e| format!("Cannot write ODS: {e}"))?;

    println!(
        "✓  Row {} written to \"{}\" (sheet: {})",
        next_row,
        ods_path.display(),
        SHEET_NAME
    );
    Ok(())
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <pr-YYYYMMDDHHMI.txt> [output.ods]", args[0]);
        eprintln!("  Default output: PR Consolidated.ods");
        std::process::exit(1);
    }

    let txt_path = Path::new(&args[1]);
    let ods_path_str = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("PR Consolidated.ods");
    let ods_path = Path::new(ods_path_str);

    println!("Reading: {}", txt_path.display());

    let stats = match parse_ping_file(txt_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    println!("  Report:      {}", stats.report_dt);
    println!("  Host:        {}", stats.host);
    println!("  Transmitted: {}", stats.transmitted);
    println!("  Received:    {}", stats.received);
    println!("  Loss:        {}%", stats.loss_pct);
    println!(
        "  RTT:         min={} avg={} max={} mdev={} ms",
        stats.rtt_min, stats.rtt_avg, stats.rtt_max, stats.rtt_mdev
    );

    if let Err(e) = append_to_ods(ods_path, &stats) {
        eprintln!("Error writing ODS: {e}");
        std::process::exit(1);
    }
}

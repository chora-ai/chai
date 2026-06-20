//! Logs CLI: read and search the gateway's in-memory log buffer via the HTTP API.

use anyhow::{Context, Result};
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum LogsCmd {
    /// Return the most recent N lines from the gateway log buffer.
    Recent {
        /// Number of recent lines to return (default: 50, max: 200)
        #[arg(long, default_value_t = 50)]
        lines: usize,
        /// Filter by log level (info, warn, error, debug)
        #[arg(long)]
        level: Option<String>,
    },
    /// Search the gateway log buffer for a pattern.
    Search {
        /// Pattern to search for (substring match)
        #[arg(long, allow_hyphen_values = true)]
        pattern: String,
        /// Number of context lines around each match (default: 2)
        #[arg(long, default_value_t = 2)]
        context: usize,
    },
}

/// Resolve the gateway base URL from the active profile config, defaulting to
/// `http://127.0.0.1:15151`.
fn gateway_base_url() -> String {
    let port = lib::config::load_config(None)
        .map(|(c, _)| c.gateway.port)
        .unwrap_or(15151);
    format!("http://127.0.0.1:{}", port)
}

/// Fetch all log lines from the gateway's /logs endpoint.
fn fetch_logs() -> Result<Vec<String>> {
    let base = gateway_base_url();
    let url = format!("{}/logs?afterSeq=0&lines=1000", base);
    let output = std::process::Command::new("curl")
        .args(["-sf", "--max-time", "5", &url])
        .output()
        .context("failed to run curl — is it installed?")?;

    if !output.status.success() {
        anyhow::bail!(
            "failed to reach gateway at {} (is it running?): {}",
            base,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let resp: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("failed to parse gateway /logs response")?;

    let lines = resp["lines"]
        .as_array()
        .context("unexpected /logs response format")?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    Ok(lines)
}

pub(crate) fn run_logs(cmd: LogsCmd) -> Result<()> {
    match cmd {
        LogsCmd::Recent { lines, level } => {
            let all_lines = fetch_logs()?;
            let lines = lines.min(200);

            let filtered: Vec<&String> = all_lines
                .iter()
                .rev()
                .filter(|line| {
                    if let Some(ref lvl) = level {
                        let lvl_upper = lvl.to_uppercase();
                        // Log format: [timestamp LEVEL gateway] msg
                        line.contains(&format!(" {} ", lvl_upper))
                            || line.contains(&format!(" {} gateway", lvl_upper))
                    } else {
                        true
                    }
                })
                .take(lines)
                .collect();

            if filtered.is_empty() {
                println!("no log lines matching filter");
                return Ok(());
            }

            for line in filtered.into_iter().rev() {
                println!("{}", line);
            }
            Ok(())
        }
        LogsCmd::Search { pattern, context } => {
            let all_lines = fetch_logs()?;

            if all_lines.is_empty() {
                println!("no log lines in buffer");
                return Ok(());
            }

            // Find matching line indices
            let matches: Vec<usize> = all_lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.contains(&pattern))
                .map(|(i, _)| i)
                .collect();

            if matches.is_empty() {
                println!("0 matches for pattern");
                return Ok(());
            }

            // Build ranges with context, merging overlapping ranges
            let mut ranges: Vec<(usize, usize)> = Vec::new();
            for &idx in &matches {
                let start = idx.saturating_sub(context);
                let end = (idx + context).min(all_lines.len() - 1);
                if let Some(last) = ranges.last_mut() {
                    if start <= last.1 + 1 {
                        last.1 = end;
                        continue;
                    }
                }
                ranges.push((start, end));
            }

            for (range_idx, (start, end)) in ranges.iter().enumerate() {
                if range_idx > 0 {
                    println!("---");
                }
                for i in *start..=*end {
                    if matches.binary_search(&i).is_ok() {
                        println!("> {}", all_lines[i]);
                    } else {
                        println!("  {}", all_lines[i]);
                    }
                }
            }

            println!("\n{} match(es) for pattern", matches.len());
            Ok(())
        }
    }
}

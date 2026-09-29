use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::time::Instant;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "vacua-naive-baseline")]
struct Args {
    #[arg(required = true)]
    path: PathBuf,

    #[arg(long, default_value = "0")]
    min_size: String,
}

#[derive(Serialize)]
struct BaselineOutput {
    files_seen: usize,
    files_hashed: usize,
    bytes_read: u64,
    elapsed_ms: f64,
    duplicate_groups: usize,
    duplicate_members: usize,
}

fn parse_size(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    if let Some(rest) = s.strip_suffix(['k', 'K']) {
        rest.trim().parse::<u64>().unwrap_or(0) * 1024
    } else if let Some(rest) = s.strip_suffix(['m', 'M']) {
        rest.trim().parse::<u64>().unwrap_or(0) * 1024 * 1024
    } else if let Some(rest) = s.strip_suffix(['g', 'G']) {
        rest.trim().parse::<u64>().unwrap_or(0) * 1024 * 1024 * 1024
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let min_bytes = parse_size(&args.min_size);
    let start = Instant::now();

    let mut files_seen = 0;
    let mut files_hashed = 0;
    let mut bytes_read = 0u64;
    let mut hash_map: HashMap<blake3::Hash, Vec<PathBuf>> = HashMap::new();

    let mut buffer = vec![0u8; 64 * 1024];

    for entry in WalkDir::new(&args.path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let ft = entry.file_type();
        if !ft.is_file() {
            continue;
        }
        files_seen += 1;
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.len() < min_bytes {
            continue;
        }

        let f = match File::open(entry.path()) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut reader = BufReader::with_capacity(64 * 1024, f);
        let mut hasher = blake3::Hasher::new();

        let mut file_bytes = 0u64;
        loop {
            let n = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            hasher.update(&buffer[..n]);
            file_bytes += n as u64;
        }

        bytes_read += file_bytes;
        files_hashed += 1;
        let hash = hasher.finalize();
        hash_map.entry(hash).or_default().push(entry.into_path());
    }

    let elapsed = start.elapsed();
    let duplicate_groups = hash_map.values().filter(|v| v.len() > 1).count();
    let duplicate_members: usize = hash_map
        .values()
        .filter(|v| v.len() > 1)
        .map(|v| v.len())
        .sum();

    let out = BaselineOutput {
        files_seen,
        files_hashed,
        bytes_read,
        elapsed_ms: elapsed.as_secs_f64() * 1000.0,
        duplicate_groups,
        duplicate_members,
    };

    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

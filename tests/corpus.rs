use oxpdf::Document;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Default)]
struct CorpusMetrics {
    total: usize,
    passed: usize,
    failed: usize,
    panics: usize,
    failures_by_type: BTreeMap<String, usize>,
    load_times_ms: Vec<f64>,
    peak_rss_kb: usize,
}

fn collect_pdfs<P: AsRef<Path>>(dir: P, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_pdfs(&path, files);
            } else if path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
            {
                files.push(path);
            }
        }
    }
}

fn read_proc_rss_kb() -> usize {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmHWM:") || line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            return kb;
                        }
                    }
                }
            }
        }
    }
    #[cfg(windows)]
    {
        #[repr(C)]
        struct PROCESS_MEMORY_COUNTERS {
            cb: u32,
            page_fault_count: u32,
            peak_working_set_size: usize,
            working_set_size: usize,
            quota_peak_paged_pool_usage: usize,
            quota_paged_pool_usage: usize,
            quota_peak_non_paged_pool_usage: usize,
            quota_non_paged_pool_usage: usize,
            pagefile_usage: usize,
            peak_pagefile_usage: usize,
        }
        #[link(name = "psapi")]
        extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessMemoryInfo(
                process: *mut std::ffi::c_void,
                counters: *mut PROCESS_MEMORY_COUNTERS,
                cb: u32,
            ) -> i32;
        }
        unsafe {
            let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0 {
                return counters.working_set_size / 1024;
            }
        }
    }
    0
}

#[test]
fn test_corpus_verification_harness() {
    let mut files = Vec::new();
    let corpus_dirs = ["corpus/verapdf_repo", "corpus/pdfjs", "corpus"];

    for dir in &corpus_dirs {
        if Path::new(dir).exists() {
            collect_pdfs(dir, &mut files);
        }
    }

    if files.is_empty() {
        println!("Note: Corpus directory not present. Skipping full corpus walk.");
        return;
    }

    let mut metrics = CorpusMetrics::default();

    for file in &files {
        metrics.total += 1;

        let data = match fs::read(file) {
            Ok(d) => d,
            Err(_) => {
                metrics.failed += 1;
                *metrics
                    .failures_by_type
                    .entry("IoError".to_string())
                    .or_insert(0) += 1;
                continue;
            }
        };

        // Enforce zero panics via catch_unwind
        let t0 = Instant::now();
        let load_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Document::load(&data)));
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        metrics.load_times_ms.push(elapsed_ms);

        let cur_rss = read_proc_rss_kb();
        if cur_rss > metrics.peak_rss_kb {
            metrics.peak_rss_kb = cur_rss;
        }

        match load_result {
            Ok(Ok(_)) => {
                metrics.passed += 1;
            }
            Ok(Err(err)) => {
                metrics.failed += 1;
                let variant_name = match &err {
                    oxpdf::Error::UnexpectedEof(_) => "UnexpectedEof",
                    oxpdf::Error::SyntaxError { .. } => "SyntaxError",
                    oxpdf::Error::InvalidNumber(_) => "InvalidNumber",
                    oxpdf::Error::InvalidHexString(_) => "InvalidHexString",
                    oxpdf::Error::UnterminatedString(_) => "UnterminatedString",
                    oxpdf::Error::RecursionLimitExceeded(_) => "RecursionLimitExceeded",
                    oxpdf::Error::BrokenXref { .. } => "BrokenXref",
                    oxpdf::Error::CyclicReference { .. } => "CyclicReference",
                    oxpdf::Error::TruncatedFile { .. } => "TruncatedFile",
                    oxpdf::Error::UnsupportedFilter { .. } => "UnsupportedFilter",
                    oxpdf::Error::MissingTrailer => "MissingTrailer",
                    oxpdf::Error::RecoveryFailed { .. } => "RecoveryFailed",
                    oxpdf::Error::Io(_) => "Io",
                    oxpdf::Error::Unsupported(_) => "Unsupported",
                    oxpdf::Error::Decryption(_) => "Decryption",
                };
                *metrics
                    .failures_by_type
                    .entry(variant_name.to_string())
                    .or_insert(0) += 1;
            }
            Err(_) => {
                metrics.panics += 1;
                metrics.failed += 1;
                *metrics
                    .failures_by_type
                    .entry("Panics".to_string())
                    .or_insert(0) += 1;
            }
        }
    }

    metrics
        .load_times_ms
        .sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = if metrics.load_times_ms.is_empty() {
        0.0
    } else {
        metrics.load_times_ms[metrics.load_times_ms.len() * 50 / 100]
    };
    let p99 = if metrics.load_times_ms.is_empty() {
        0.0
    } else {
        metrics.load_times_ms[metrics.load_times_ms.len() * 99 / 100]
    };

    let pass_pct = if metrics.total > 0 {
        (metrics.passed as f64 / metrics.total as f64) * 100.0
    } else {
        100.0
    };
    let fail_pct = if metrics.total > 0 {
        (metrics.failed as f64 / metrics.total as f64) * 100.0
    } else {
        0.0
    };

    // Print final output format matching §6 specification verbatim
    println!("\nTotal files: {}", metrics.total);
    println!("Passed: {} ({:.1}%)", metrics.passed, pass_pct);
    println!("Failed: {} ({:.1}%)", metrics.failed, fail_pct);
    for (err_type, count) in &metrics.failures_by_type {
        println!("  {}: {}", err_type, count);
    }
    println!("  Panics: {}", metrics.panics);
    println!("p50 load time: {:.2}ms   p99 load time: {:.2}ms", p50, p99);
    println!(
        "Peak RSS (largest file): {:.2} MB",
        metrics.peak_rss_kb as f64 / 1024.0
    );

    // Hard release gate: Panics must be 0
    assert_eq!(
        metrics.panics, 0,
        "RELEASE GATE FAILED: Corpus encountered panics!"
    );
}

use oxpdf::Document as OxDocument;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

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

fn get_peak_rss_kb() -> usize {
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
                return counters.peak_working_set_size / 1024;
            }
        }
    }
    0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let engine = args.get(1).map(|s| s.as_str()).unwrap_or("oxpdf");

    // Find the largest file in corpus dynamically
    let mut files = Vec::new();
    collect_pdfs("corpus", &mut files);
    let mut largest_file = PathBuf::new();
    let mut max_len = 0u64;
    for f in files {
        if let Ok(meta) = fs::metadata(&f) {
            if meta.len() > max_len {
                max_len = meta.len();
                largest_file = f;
            }
        }
    }
    let data = fs::read(&largest_file).expect("failed to read largest file");
    let file_len = data.len();

    let baseline_rss = get_peak_rss_kb();
    let start = Instant::now();

    match engine {
        "oxpdf" => {
            let doc = OxDocument::load(&data).expect("oxpdf load failed");
            let count = doc.object_count();
            let elapsed = start.elapsed();
            let peak_rss = get_peak_rss_kb();
            println!("ENGINE: oxpdf");
            println!("FILE_BYTES: {}", file_len);
            println!("OBJECTS: {}", count);
            println!("TIME_US: {}", elapsed.as_micros());
            println!("TIME_MS: {:.3}", elapsed.as_secs_f64() * 1000.0);
            println!("BASELINE_RSS_KB: {}", baseline_rss);
            println!("PEAK_RSS_KB: {}", peak_rss);
            println!("PEAK_RSS_MB: {:.2}", peak_rss as f64 / 1024.0);
        }
        "lopdf" => {
            let doc = lopdf::Document::load_mem(&data).expect("lopdf load failed");
            let count = doc.objects.len();
            let elapsed = start.elapsed();
            let peak_rss = get_peak_rss_kb();
            println!("ENGINE: lopdf");
            println!("FILE_BYTES: {}", file_len);
            println!("OBJECTS: {}", count);
            println!("TIME_US: {}", elapsed.as_micros());
            println!("TIME_MS: {:.3}", elapsed.as_secs_f64() * 1000.0);
            println!("BASELINE_RSS_KB: {}", baseline_rss);
            println!("PEAK_RSS_KB: {}", peak_rss);
            println!("PEAK_RSS_MB: {:.2}", peak_rss as f64 / 1024.0);
        }
        _ => eprintln!("unknown engine"),
    }
}

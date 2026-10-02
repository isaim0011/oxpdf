use oxpdf::{Document, MmapSource, PdfSource};
use std::fs::File;

fn get_peak_rss_kb() -> usize {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: measure_memory <path_to_pdf>");
        std::process::exit(1);
    }

    let path = &args[1];
    println!("Opening `{}` via MmapSource...", path);

    let file = File::open(path)?;
    let source = MmapSource::open(&file)?;
    println!(
        "Mmap created successfully. File size: {:.2} MB",
        source.len() as f64 / (1024.0 * 1024.0)
    );

    let slice = source.as_slice()?;
    let baseline_rss = get_peak_rss_kb();
    let start = std::time::Instant::now();
    let doc = Document::load(slice)?;
    let load_time = start.elapsed();

    println!("Document loaded successfully.");
    println!("Indexing time: {:.3} ms", load_time.as_secs_f64() * 1000.0);
    println!("Object count: {}", doc.object_count());
    if let Ok(count) = doc.page_count() {
        println!("Page count: {}", count);
    }

    // Access individual objects to verify zero-copy lookup
    for id in 1..=doc.object_count().min(10) as u32 {
        if let Ok(Some(obj)) = doc.get_object(id) {
            match obj {
                oxpdf::Object::Stream { dict, data } => {
                    println!("  Object {}: Stream (dict keys: {:?}, payload: {} bytes)", id, dict.keys().collect::<Vec<_>>(), data.len());
                }
                other => {
                    println!("  Object {}: {:?}", id, other);
                }
            }
        }
    }

    let peak_rss = get_peak_rss_kb();
    println!("Baseline RSS: {:.2} MB", baseline_rss as f64 / 1024.0);
    println!("Peak RSS: {:.2} MB", peak_rss as f64 / 1024.0);
    println!("Net heap delta: {:.2} KB", (peak_rss.saturating_sub(baseline_rss)) as f64);

    Ok(())
}

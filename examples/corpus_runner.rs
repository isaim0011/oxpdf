use oxpdf::Document as OxDocument;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Default, Clone)]
struct RunMetrics {
    engine_name: String,
    total_files: usize,
    total_bytes: u64,
    passed: usize,
    failed: usize,
    panics: usize,
    total_time_secs: f64,
    load_times_ms: Vec<f64>,
    peak_rss_kb: usize,
    largest_file_name: String,
    largest_file_bytes: u64,
    largest_file_time_ms: f64,
    largest_file_rss_kb: usize,
    failures_by_type: BTreeMap<String, usize>,
}

impl RunMetrics {
    fn new(name: &str) -> Self {
        Self {
            engine_name: name.to_string(),
            ..Default::default()
        }
    }

    fn print_summary(&self) {
        let pass_pct = if self.total_files > 0 {
            (self.passed as f64 / self.total_files as f64) * 100.0
        } else {
            0.0
        };
        let fail_pct = if self.total_files > 0 {
            (self.failed as f64 / self.total_files as f64) * 100.0
        } else {
            0.0
        };

        let mb_total = self.total_bytes as f64 / (1024.0 * 1024.0);
        let throughput_mb_s = if self.total_time_secs > 0.0 {
            mb_total / self.total_time_secs
        } else {
            0.0
        };
        let throughput_files_s = if self.total_time_secs > 0.0 {
            self.total_files as f64 / self.total_time_secs
        } else {
            0.0
        };

        let mut sorted_times = self.load_times_ms.clone();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let count = sorted_times.len();
        let (min_ms, max_ms, avg_ms, p50, p90, p99) = if count > 0 {
            let min = sorted_times[0];
            let max = sorted_times[count - 1];
            let sum: f64 = sorted_times.iter().sum();
            let avg = sum / count as f64;
            let p50 = sorted_times[count * 50 / 100];
            let p90 = sorted_times[(count as f64 * 0.90) as usize % count];
            let p99 = sorted_times[((count as f64 * 0.99).ceil() as usize)
                .saturating_sub(1)
                .min(count - 1)];
            (min, max, avg, p50, p90, p99)
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        };

        println!("------------------------------------------------------------");
        println!("Engine:               {}", self.engine_name);
        println!("Total files scanned:  {}", self.total_files);
        println!(
            "Total volume:         {:.2} MB ({} bytes)",
            mb_total, self.total_bytes
        );
        println!("Wall-clock duration:  {:.3} s", self.total_time_secs);
        println!("Passed:               {} ({:.1}%)", self.passed, pass_pct);
        println!("Failed:               {} ({:.1}%)", self.failed, fail_pct);
        println!("Panics:               {}", self.panics);
        println!(
            "Throughput:           {:.2} MB/s | {:.1} files/sec",
            throughput_mb_s, throughput_files_s
        );
        println!("Latency (min / avg):  {:.3} ms / {:.3} ms", min_ms, avg_ms);
        println!("Latency p50 (median): {:.3} ms", p50);
        println!("Latency p90:          {:.3} ms", p90);
        println!("Latency p99:          {:.3} ms", p99);
        println!("Latency max:          {:.3} ms", max_ms);
        println!(
            "Peak Process RSS:     {:.2} MB",
            self.peak_rss_kb as f64 / 1024.0
        );
        if !self.largest_file_name.is_empty() {
            println!(
                "Largest File:         {} ({:.2} MB)",
                self.largest_file_name,
                self.largest_file_bytes as f64 / (1024.0 * 1024.0)
            );
            println!("Largest File Latency: {:.2} ms", self.largest_file_time_ms);
            println!(
                "Largest File RSS:     {:.2} MB",
                self.largest_file_rss_kb as f64 / 1024.0
            );
        }
        if !self.failures_by_type.is_empty() {
            println!("\nTop Failure Categories:");
            let mut sorted_failures: Vec<_> = self.failures_by_type.iter().collect();
            sorted_failures.sort_by_key(|a| std::cmp::Reverse(a.1));
            for (err_type, cnt) in sorted_failures.iter().take(10) {
                println!("  [{:>4}] {}", cnt, err_type);
            }
        }
        println!("------------------------------------------------------------");
    }
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
                return counters.working_set_size / 1024;
            }
        }
    }
    0
}

enum Engine {
    Oxpdf,
    Lopdf,
}

fn run_engine_benchmark(
    engine: Engine,
    pdf_files: &[PathBuf],
    preloaded_data: &[Vec<u8>],
    verbose: bool,
) -> RunMetrics {
    let engine_name = match engine {
        Engine::Oxpdf => "oxpdf (v0.4.0)",
        Engine::Lopdf => "lopdf (v0.36.0)",
    };

    let mut metrics = RunMetrics::new(engine_name);
    metrics.total_files = pdf_files.len();

    // Identify largest file
    let mut largest_idx = 0;
    let mut max_bytes = 0;
    for (i, data) in preloaded_data.iter().enumerate() {
        metrics.total_bytes += data.len() as u64;
        if data.len() as u64 > max_bytes {
            max_bytes = data.len() as u64;
            largest_idx = i;
        }
    }

    if !pdf_files.is_empty() {
        metrics.largest_file_name = pdf_files[largest_idx]
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        metrics.largest_file_bytes = max_bytes;
    }

    let overall_start = Instant::now();

    for (idx, (file, data)) in pdf_files.iter().zip(preloaded_data.iter()).enumerate() {
        let is_largest = idx == largest_idx;
        let rss_before = if is_largest { get_peak_rss_kb() } else { 0 };

        let t0 = Instant::now();
        let parse_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match engine {
                Engine::Oxpdf => {
                    let doc = OxDocument::load(data).map_err(|e| format!("{:?}", e))?;
                    let _ = doc.object_count();
                    Ok::<(), String>(())
                }
                Engine::Lopdf => {
                    let doc = lopdf::Document::load_mem(data).map_err(|e| format!("{:?}", e))?;
                    let _ = doc.objects.len();
                    Ok::<(), String>(())
                }
            }));
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        metrics.load_times_ms.push(elapsed_ms);

        let cur_rss = get_peak_rss_kb();
        if cur_rss > metrics.peak_rss_kb {
            metrics.peak_rss_kb = cur_rss;
        }

        if is_largest {
            metrics.largest_file_time_ms = elapsed_ms;
            let rss_after = get_peak_rss_kb();
            metrics.largest_file_rss_kb = if rss_after > rss_before {
                rss_after - rss_before
            } else {
                rss_after
            };
        }

        match parse_result {
            Ok(Ok(())) => {
                metrics.passed += 1;
                if verbose {
                    println!("[PASS] {:<50} in {:>8.2} ms", file.display(), elapsed_ms);
                }
            }
            Ok(Err(err_msg)) => {
                metrics.failed += 1;
                let short_err = err_msg.lines().next().unwrap_or("UnknownError").to_string();
                *metrics.failures_by_type.entry(short_err).or_insert(0) += 1;
                if verbose {
                    println!(
                        "[FAIL] {:<50} in {:>8.2} ms: {}",
                        file.display(),
                        elapsed_ms,
                        err_msg
                    );
                }
            }
            Err(_) => {
                metrics.panics += 1;
                metrics.failed += 1;
                *metrics
                    .failures_by_type
                    .entry("Panic".to_string())
                    .or_insert(0) += 1;
                if verbose {
                    println!("[PANIC] {:<50}", file.display());
                }
            }
        }
    }

    metrics.total_time_secs = overall_start.elapsed().as_secs_f64();
    metrics
}

fn run_engine_cold(engine: Engine, pdf_files: &[PathBuf], verbose: bool) -> RunMetrics {
    let engine_name = match engine {
        Engine::Oxpdf => "oxpdf (v0.4.0) [Cold]",
        Engine::Lopdf => "lopdf (v0.36.0) [Cold]",
    };

    let mut metrics = RunMetrics::new(engine_name);
    metrics.total_files = pdf_files.len();

    let overall_start = Instant::now();

    for file in pdf_files {
        let t0 = Instant::now();
        let parse_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match engine {
                Engine::Oxpdf => {
                    let data = fs::read(file).map_err(|e| format!("{:?}", e))?;
                    let doc = OxDocument::load(&data).map_err(|e| format!("{:?}", e))?;
                    let _ = doc.object_count();
                    Ok::<usize, String>(data.len())
                }
                Engine::Lopdf => {
                    let data = fs::read(file).map_err(|e| format!("{:?}", e))?;
                    let doc = lopdf::Document::load_mem(&data).map_err(|e| format!("{:?}", e))?;
                    let _ = doc.objects.len();
                    Ok::<usize, String>(data.len())
                }
            }));
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        metrics.load_times_ms.push(elapsed_ms);

        let cur_rss = get_peak_rss_kb();
        if cur_rss > metrics.peak_rss_kb {
            metrics.peak_rss_kb = cur_rss;
        }

        match parse_result {
            Ok(Ok(bytes)) => {
                metrics.total_bytes += bytes as u64;
                metrics.passed += 1;
                if verbose {
                    println!("[PASS] {:<50} in {:>8.2} ms", file.display(), elapsed_ms);
                }
            }
            Ok(Err(err_msg)) => {
                metrics.failed += 1;
                let short_err = err_msg.lines().next().unwrap_or("UnknownError").to_string();
                *metrics.failures_by_type.entry(short_err).or_insert(0) += 1;
            }
            Err(_) => {
                metrics.panics += 1;
                metrics.failed += 1;
                *metrics
                    .failures_by_type
                    .entry("Panic".to_string())
                    .or_insert(0) += 1;
            }
        }
    }

    metrics.total_time_secs = overall_start.elapsed().as_secs_f64();
    metrics
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut target = "5820".to_string();
    let mut engine_arg = "both".to_string();
    let mut verbose = false;
    let mut cold_mode = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--engine" if i + 1 < args.len() => {
                engine_arg = args[i + 1].to_lowercase();
                i += 2;
            }
            "--corpus" if i + 1 < args.len() => {
                target = args[i + 1].clone();
                i += 2;
            }
            "--cold" => {
                cold_mode = true;
                i += 1;
            }
            "--verbose" | "-v" => {
                verbose = true;
                i += 1;
            }
            arg if !arg.starts_with("--") => {
                target = arg.to_string();
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    println!("============================================================");
    println!("oxpdf vs lopdf Comparative Performance Benchmark");
    println!("Target Mode: {}", target);
    println!("============================================================");

    let mut pdf_files = Vec::new();
    if target == "5820" || target == "corpus-5820" {
        // Collect exact 5,820 corpus directories as in tests/corpus.rs
        let corpus_dirs = ["corpus/verapdf_repo", "corpus/pdfjs", "corpus"];
        for dir in &corpus_dirs {
            if Path::new(dir).exists() {
                collect_pdfs(dir, &mut pdf_files);
            }
        }
    } else if target == "unique" || target == "corpus-2910" {
        collect_pdfs("corpus", &mut pdf_files);
    } else {
        collect_pdfs(&target, &mut pdf_files);
    }

    if pdf_files.is_empty() {
        eprintln!("No PDF files found for target `{}`.", target);
        return;
    }

    let mut oxpdf_metrics = None;
    let mut lopdf_metrics = None;

    if cold_mode {
        println!("Running in COLD mode: reading each file on-the-fly from disk...");
        if engine_arg == "oxpdf" || engine_arg == "both" {
            println!("\n>>> Running oxpdf [Cold] benchmark...");
            let m = run_engine_cold(Engine::Oxpdf, &pdf_files, verbose);
            m.print_summary();
            oxpdf_metrics = Some(m);
        }

        if engine_arg == "lopdf" || engine_arg == "both" {
            println!("\n>>> Running lopdf [Cold] benchmark...");
            let m = run_engine_cold(Engine::Lopdf, &pdf_files, verbose);
            m.print_summary();
            lopdf_metrics = Some(m);
        }
    } else {
        println!(
            "Pre-loading {} files into memory (warm cache)...",
            pdf_files.len()
        );
        let preload_start = Instant::now();
        let mut preloaded_data = Vec::with_capacity(pdf_files.len());
        let mut total_bytes = 0u64;
        for file in &pdf_files {
            match fs::read(file) {
                Ok(bytes) => {
                    total_bytes += bytes.len() as u64;
                    preloaded_data.push(bytes);
                }
                Err(e) => {
                    eprintln!("Warning: failed to read {}: {}", file.display(), e);
                    preloaded_data.push(Vec::new());
                }
            }
        }
        println!(
            "Pre-loaded {} files ({:.2} MB) in {:.2} s",
            preloaded_data.len(),
            total_bytes as f64 / (1024.0 * 1024.0),
            preload_start.elapsed().as_secs_f64()
        );

        if engine_arg == "oxpdf" || engine_arg == "both" {
            println!("\n>>> Running oxpdf benchmark...");
            let m = run_engine_benchmark(Engine::Oxpdf, &pdf_files, &preloaded_data, verbose);
            m.print_summary();
            oxpdf_metrics = Some(m);
        }

        if engine_arg == "lopdf" || engine_arg == "both" {
            println!("\n>>> Running lopdf benchmark...");
            let m = run_engine_benchmark(Engine::Lopdf, &pdf_files, &preloaded_data, verbose);
            m.print_summary();
            lopdf_metrics = Some(m);
        }
    }

    if let (Some(ox), Some(lo)) = (oxpdf_metrics, lopdf_metrics) {
        println!("\n============================================================");
        println!("HEAD-TO-HEAD COMPARISON MATRIX");
        println!("============================================================");
        println!(
            "{:<24} | {:<18} | {:<18} | {:<10}",
            "Metric", "oxpdf (v0.4.0)", "lopdf (v0.36.0)", "Ratio (ox/lo)"
        );
        println!("{:-<76}", "");

        let ox_mb_s = (ox.total_bytes as f64 / (1024.0 * 1024.0)) / ox.total_time_secs;
        let lo_mb_s = (lo.total_bytes as f64 / (1024.0 * 1024.0)) / lo.total_time_secs;
        let thrpt_ratio = if lo_mb_s > 0.0 {
            ox_mb_s / lo_mb_s
        } else {
            0.0
        };

        let ox_fps = ox.total_files as f64 / ox.total_time_secs;
        let lo_fps = lo.total_files as f64 / lo.total_time_secs;
        let fps_ratio = if lo_fps > 0.0 { ox_fps / lo_fps } else { 0.0 };

        let mut ox_sorted = ox.load_times_ms.clone();
        ox_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let ox_p50 = ox_sorted[ox_sorted.len() * 50 / 100];
        let ox_p90 = ox_sorted[(ox_sorted.len() as f64 * 0.90) as usize % ox_sorted.len()];
        let ox_p99 = ox_sorted[((ox_sorted.len() as f64 * 0.99).ceil() as usize)
            .saturating_sub(1)
            .min(ox_sorted.len() - 1)];

        let mut lo_sorted = lo.load_times_ms.clone();
        lo_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let lo_p50 = lo_sorted[lo_sorted.len() * 50 / 100];
        let lo_p90 = lo_sorted[(lo_sorted.len() as f64 * 0.90) as usize % lo_sorted.len()];
        let lo_p99 = lo_sorted[((lo_sorted.len() as f64 * 0.99).ceil() as usize)
            .saturating_sub(1)
            .min(lo_sorted.len() - 1)];

        let ox_rss_mb = ox.peak_rss_kb as f64 / 1024.0;
        let lo_rss_mb = lo.peak_rss_kb as f64 / 1024.0;
        let rss_ratio = if ox_rss_mb > 0.0 {
            lo_rss_mb / ox_rss_mb
        } else {
            0.0
        };

        println!(
            "{:<24} | {:>15.2} MB/s | {:>15.2} MB/s | {:>8.2}x",
            "Throughput (MB/s)", ox_mb_s, lo_mb_s, thrpt_ratio
        );
        println!(
            "{:<24} | {:>14.1} files/s| {:>14.1} files/s| {:>8.2}x",
            "Speed (files/sec)", ox_fps, lo_fps, fps_ratio
        );
        println!(
            "{:<24} | {:>15.3} ms   | {:>15.3} ms   | {:>8.2}x",
            "p50 Latency (median)",
            ox_p50,
            lo_p50,
            lo_p50 / ox_p50
        );
        println!(
            "{:<24} | {:>15.3} ms   | {:>15.3} ms   | {:>8.2}x",
            "p90 Latency",
            ox_p90,
            lo_p90,
            lo_p90 / ox_p90
        );
        println!(
            "{:<24} | {:>15.3} ms   | {:>15.3} ms   | {:>8.2}x",
            "p99 Latency",
            ox_p99,
            lo_p99,
            lo_p99 / ox_p99
        );
        println!(
            "{:<24} | {:>15.2} MB   | {:>15.2} MB   | {:>8.2}x less",
            "Peak Process RSS", ox_rss_mb, lo_rss_mb, rss_ratio
        );
        println!(
            "{:<24} | {:>15.2} MB   | {:>15.2} MB   | {:>8.2}x less",
            "Largest File RSS Delta",
            ox.largest_file_rss_kb as f64 / 1024.0,
            lo.largest_file_rss_kb as f64 / 1024.0,
            if ox.largest_file_rss_kb > 0 {
                lo.largest_file_rss_kb as f64 / ox.largest_file_rss_kb as f64
            } else {
                0.0
            }
        );
        println!(
            "{:<24} | {:>18} | {:>18} |",
            "Panics Count", ox.panics, lo.panics
        );
        println!("============================================================");
    }
}

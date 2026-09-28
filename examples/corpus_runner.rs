use oxpdf::Document;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Default)]
struct CorpusSummary {
    total: usize,
    passed: usize,
    failed: usize,
    failures_by_type: BTreeMap<String, usize>,
    load_times_ms: Vec<f64>,
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
    // RSS measurement via K32GetProcessMemoryCounters requires psapi linkage
    // which is unavailable in the mingw toolchain. Return 0 on Windows;
    // measure separately with Task Manager / perfmon if needed.
    #[cfg(target_family = "unix")]
    {
        use std::mem::MaybeUninit;
        unsafe {
            let mut usage = MaybeUninit::<libc::rusage>::uninit();
            if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) == 0 {
                let u = usage.assume_init();
                #[cfg(target_os = "macos")]
                return (u.ru_maxrss / 1024) as usize;
                #[cfg(not(target_os = "macos"))]
                return u.ru_maxrss as usize;
            }
        }
    }
    0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let target_dir = if args.len() > 1 {
        args[1].clone()
    } else {
        "corpus".to_string()
    };

    println!("============================================================");
    println!("oxpdf Real-World Corpus Verification Harness");
    println!("Scanning directory: {}", target_dir);
    println!("============================================================");

    let mut pdf_files = Vec::new();
    collect_pdfs(&target_dir, &mut pdf_files);

    if pdf_files.is_empty() {
        println!("No PDF files found in `{}`. Please run scripts/fetch_corpus.sh or fetch_corpus.ps1 first.", target_dir);
        return;
    }

    let mut summary = CorpusSummary::default();

    println!(
        "{:<45} | {:<8} | {:<7} | {:<7} | {:<10} | {:<12}",
        "File", "Status", "Objects", "Pages", "Time (ms)", "Peak RSS (KB)"
    );
    println!("{:-<105}", "");

    for file in &pdf_files {
        summary.total += 1;
        let file_name = file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let display_name = if file_name.len() > 42 {
            format!("{}...", &file_name[..39])
        } else {
            file_name.to_string()
        };

        let data = match fs::read(file) {
            Ok(d) => d,
            Err(e) => {
                summary.failed += 1;
                *summary
                    .failures_by_type
                    .entry(format!("ReadError: {}", e))
                    .or_insert(0) += 1;
                println!(
                    "{:<45} | {:<8} | {:<7} | {:<7} | {:<10} | {:<12}",
                    display_name, "FAIL", "-", "-", "-", "-"
                );
                continue;
            }
        };

        let start = Instant::now();
        match Document::load(&data) {
            Ok(doc) => {
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                let obj_count = doc.object_count();
                let page_count = doc.page_count().unwrap_or(0);
                let peak_rss = get_peak_rss_kb();

                summary.passed += 1;
                summary.load_times_ms.push(elapsed_ms);

                println!(
                    "{:<45} | {:<8} | {:<7} | {:<7} | {:<10.2} | {:<12}",
                    display_name, "PASS", obj_count, page_count, elapsed_ms, peak_rss
                );
            }
            Err(err) => {
                summary.failed += 1;
                let err_desc = match &err {
                    oxpdf::Error::UnexpectedEof(offset) => format!("UnexpectedEof({})", offset),
                    oxpdf::Error::SyntaxError { message, .. } => {
                        format!("SyntaxError: {}", message)
                    }
                    oxpdf::Error::InvalidNumber(offset) => format!("InvalidNumber({})", offset),
                    oxpdf::Error::InvalidHexString(offset) => {
                        format!("InvalidHexString({})", offset)
                    }
                    oxpdf::Error::UnterminatedString(offset) => {
                        format!("UnterminatedString({})", offset)
                    }
                    oxpdf::Error::RecursionLimitExceeded(d) => {
                        format!("RecursionLimitExceeded({})", d)
                    }
                    oxpdf::Error::Io(s) => format!("Io({})", s),
                    oxpdf::Error::Unsupported(s) => format!("Unsupported: {}", s),
                };
                *summary
                    .failures_by_type
                    .entry(err_desc.clone())
                    .or_insert(0) += 1;

                println!(
                    "{:<45} | {:<8} | {:<7} | {:<7} | {:<10} | {:<12}",
                    display_name, "FAIL", "-", "-", "-", "-"
                );
            }
        }
    }

    println!("{:-<105}", "");
    println!("Corpus Summary:");
    println!("  Total files scanned: {}", summary.total);
    println!("  Passed:              {}", summary.passed);
    println!("  Failed:              {}", summary.failed);
    let pass_rate = if summary.total > 0 {
        (summary.passed as f64 / summary.total as f64) * 100.0
    } else {
        0.0
    };
    println!("  Pass rate:           {:.2}%", pass_rate);

    if !summary.load_times_ms.is_empty() {
        summary
            .load_times_ms
            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p50 = summary.load_times_ms[summary.load_times_ms.len() / 2];
        let p99_idx =
            ((summary.load_times_ms.len() as f64 * 0.99).ceil() as usize).saturating_sub(1);
        let p99 = summary.load_times_ms[p99_idx.min(summary.load_times_ms.len() - 1)];
        println!("  Load time p50:       {:.2} ms", p50);
        println!("  Load time p99:       {:.2} ms", p99);
    }

    if !summary.failures_by_type.is_empty() {
        println!("\nFailures Grouped by Error Type:");
        let mut sorted_failures: Vec<_> = summary.failures_by_type.into_iter().collect();
        sorted_failures.sort_by(|a, b| b.1.cmp(&a.1));
        for (err_type, count) in sorted_failures {
            println!("  [{:>3}] {}", count, err_type);
        }
    }
    println!("============================================================");
}

use std::path::{Path, PathBuf};

fn print_help() {
    println!(
        r#"oxpdf 1.0.1
High-performance, memory-bounded, zero-copy PDF engine

USAGE:
    oxpdf <SUBCOMMAND> [OPTIONS]

COMMANDS:
    inspect <file.pdf>
            Inspect PDF structure, version, object count, page count,
            trailer /Root ID, xref format (table vs stream), and linear recovery.

    extract-text <file.pdf> [--page <N>]
            Extract Unicode plaintext from all pages or a specific 1-based page.

    pack <in.pdf> <out.pdf>
            Compact objects into compressed /ObjStm streams and print byte savings.

    bench <corpus_dir>
            Benchmark Document::load() across all PDFs in a directory.
            Reports throughput (MB/s), p50/p99 latency, and error breakdown.

FLAGS:
    -h, --help       Print help information
    -V, --version    Print version information
"#
    );
}

fn extract_pdf_version(data: &[u8]) -> String {
    let scan_limit = data.len().min(1024);
    let head = &data[..scan_limit];
    if let Some(pos) = head.windows(5).position(|w| w == b"%PDF-") {
        let ver_part = &head[pos + 5..];
        let mut ver_str = String::new();
        for &b in ver_part {
            if b == b'\r' || b == b'\n' || b == b' ' || b == b'\t' {
                break;
            }
            ver_str.push(b as char);
        }
        if !ver_str.is_empty() {
            return ver_str;
        }
    }
    "Unknown".to_string()
}

fn get_catalog_version(doc: &oxpdf::Document) -> Option<String> {
    if let Ok(Some(oxpdf::Object::Dictionary(cat))) = doc.catalog() {
        if let Some(v_obj) = cat.get("Version") {
            if let Some(name) = v_obj.as_name() {
                return Some(name.to_string());
            } else if let oxpdf::Object::String(bytes) = v_obj {
                return String::from_utf8(bytes.to_vec()).ok();
            }
        }
    }
    None
}

fn detect_xref_kind(data: &[u8], doc: &oxpdf::Document, recovered: bool) -> &'static str {
    // If any entry is compressed inside an ObjStm, it is definitely a PDF 1.5+ stream xref
    let has_compressed = doc
        .xref
        .entries
        .values()
        .any(|e| matches!(e, oxpdf::XRefEntry::Compressed { .. }));
    if has_compressed {
        return "stream";
    }

    let trailer_scan_start = data.len().saturating_sub(4096);
    let tail = &data[trailer_scan_start..];
    if let Some(pos) = tail.windows(9).rposition(|w| w == b"startxref") {
        let after = &tail[pos + 9..];
        let mut num_str = String::new();
        for &b in after {
            if b.is_ascii_whitespace() {
                if !num_str.is_empty() {
                    break;
                }
            } else if b.is_ascii_digit() {
                num_str.push(b as char);
            } else {
                break;
            }
        }
        if let Ok(offset) = num_str.parse::<usize>() {
            if offset < data.len() {
                let mut target = &data[offset..];
                while let Some(&first) = target.first() {
                    if first.is_ascii_whitespace() {
                        target = &target[1..];
                    } else {
                        break;
                    }
                }
                if target.starts_with(b"xref") {
                    return "table";
                } else if target.first().map(|b| b.is_ascii_digit()).unwrap_or(false) {
                    return "stream";
                }
            }
        }
    }

    if recovered {
        "reconstructed (table)"
    } else {
        "table"
    }
}

pub fn cmd_inspect(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read '{}': {}", path, e))?;

    // Check if linear recovery was needed
    let recovery_needed = match oxpdf::Document::load_strict(&data) {
        Ok(_) => false,
        Err(_) => true,
    };

    let doc = oxpdf::Document::load(&data).map_err(|e| format!("Failed to load PDF: {}", e))?;

    let header_ver = extract_pdf_version(&data);
    let version = match get_catalog_version(&doc) {
        Some(cat_ver) if cat_ver != header_ver => {
            format!("{} (Catalog override; header: {})", cat_ver, header_ver)
        }
        Some(cat_ver) => cat_ver,
        None => header_ver,
    };

    let object_count = doc.object_count();
    let page_count_str = match doc.page_count() {
        Ok(c) => c.to_string(),
        Err(e) => format!("Error ({})", e),
    };
    let root_id_str = match doc.catalog_id() {
        Some(id) => id.to_string(),
        None => "Not found".to_string(),
    };
    let xref_kind = detect_xref_kind(&data, &doc, recovery_needed);
    let recovery_str = if recovery_needed { "yes" } else { "no" };

    println!("File:                  {}", path);
    println!("PDF version:           {}", version);
    println!("Object count:          {}", object_count);
    println!("Page count:            {}", page_count_str);
    println!("Trailer /Root ID:      {}", root_id_str);
    println!("XRef format:           {}", xref_kind);
    println!("Linear recovery:       {}", recovery_str);

    Ok(())
}

pub fn cmd_extract_text(
    path: &str,
    page_opt: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read '{}': {}", path, e))?;
    let doc = oxpdf::Document::load(&data).map_err(|e| format!("Failed to load PDF: {}", e))?;

    let page_ids = doc
        .get_page_ids()
        .map_err(|e| format!("Failed to resolve page tree: {}", e))?;
    if page_ids.is_empty() {
        return Err("Document contains no pages".into());
    }

    if let Some(page_num) = page_opt {
        if page_num == 0 || page_num > page_ids.len() {
            return Err(format!(
                "Page {} is out of range (document has {} pages)",
                page_num,
                page_ids.len()
            )
            .into());
        }
        let page_id = page_ids[page_num - 1];
        let text = doc
            .extract_text(page_id)
            .map_err(|e| format!("Text extraction failed on page {}: {}", page_num, e))?;
        print!("{}", text);
        if !text.ends_with('\n') {
            println!();
        }
    } else {
        for (idx, &page_id) in page_ids.iter().enumerate() {
            let text = doc
                .extract_text(page_id)
                .map_err(|e| format!("Text extraction failed on page {}: {}", idx + 1, e))?;
            print!("{}", text);
            if !text.ends_with('\n') {
                println!();
            }
        }
    }

    Ok(())
}

pub fn cmd_pack(in_path: &str, out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let data =
        std::fs::read(in_path).map_err(|e| format!("Failed to read '{}': {}", in_path, e))?;
    let original_size = data.len() as u64;

    let doc = oxpdf::Document::load(&data)
        .map_err(|e| format!("Failed to load input PDF: {}", e))?;

    let out_file = std::fs::File::create(out_path)
        .map_err(|e| format!("Failed to create output file '{}': {}", out_path, e))?;
    let mut writer = std::io::BufWriter::new(out_file);
    let packed_size = doc
        .write_packed(&mut writer)
        .map_err(|e| format!("Failed to write packed PDF: {}", e))?;
    writer
        .flush()
        .map_err(|e| format!("Failed to flush output: {}", e))?;

    println!("Input file:            {}", in_path);
    println!("Output file:           {}", out_path);
    println!(
        "Original size:         {} bytes ({:.2} KB)",
        original_size,
        original_size as f64 / 1024.0
    );
    println!(
        "Packed size:           {} bytes ({:.2} KB)",
        packed_size,
        packed_size as f64 / 1024.0
    );

    if packed_size <= original_size {
        let saved = original_size - packed_size;
        let pct = if original_size > 0 {
            (saved as f64 / original_size as f64) * 100.0
        } else {
            0.0
        };
        println!("Byte savings:          {:.2}% ({} bytes saved)", pct, saved);
    } else {
        let diff = packed_size - original_size;
        let pct = if original_size > 0 {
            (diff as f64 / original_size as f64) * 100.0
        } else {
            0.0
        };
        println!("Size difference:       +{:.2}% ({} bytes added)", pct, diff);
    }

    Ok(())
}

fn collect_pdf_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_pdf_files(&path, files);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("pdf") {
                        files.push(path);
                    }
                }
            }
        }
    }
}

pub fn cmd_bench(corpus_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir_path = Path::new(corpus_dir);
    if !dir_path.exists() || !dir_path.is_dir() {
        return Err(format!("'{}' is not a valid directory", corpus_dir).into());
    }

    let mut files = Vec::new();
    collect_pdf_files(dir_path, &mut files);

    if files.is_empty() {
        println!("No PDF files found in '{}'", corpus_dir);
        return Ok(());
    }

    println!(
        "Benchmarking {} PDF files in '{}'...\n",
        files.len(),
        corpus_dir
    );

    let mut durations: Vec<std::time::Duration> = Vec::with_capacity(files.len());
    let mut total_bytes: u64 = 0;
    let mut total_parse_duration = std::time::Duration::ZERO;
    let mut error_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut success_count: usize = 0;

    for file_path in &files {
        let bytes = match std::fs::read(file_path) {
            Ok(b) => b,
            Err(e) => {
                *error_counts.entry(format!("IO error: {}", e)).or_insert(0) += 1;
                continue;
            }
        };
        total_bytes += bytes.len() as u64;

        let t0 = std::time::Instant::now();
        let res = oxpdf::Document::load(&bytes);
        let dt = t0.elapsed();

        durations.push(dt);
        total_parse_duration += dt;

        match res {
            Ok(_) => success_count += 1,
            Err(e) => {
                let key = match e {
                    oxpdf::Error::Unsupported(msg) => format!("Unsupported ({})", msg),
                    oxpdf::Error::SyntaxError { message, .. } => {
                        format!("Syntax error ({})", message)
                    }
                    oxpdf::Error::UnexpectedEof(_) => "Unexpected EOF".to_string(),
                    oxpdf::Error::RecoveryFailed { .. } => "Recovery failed".to_string(),
                    oxpdf::Error::CyclicReference { .. } => "Cyclic reference".to_string(),
                    _ => format!("{}", e),
                };
                *error_counts.entry(key).or_insert(0) += 1;
            }
        }
    }

    durations.sort_unstable();
    let count = durations.len();
    let (p50, p99) = if count > 0 {
        let idx_50 = count * 50 / 100;
        let idx_99 = (count * 99 / 100).min(count - 1);
        (durations[idx_50], durations[idx_99])
    } else {
        (std::time::Duration::ZERO, std::time::Duration::ZERO)
    };

    let throughput_mb_s = if total_parse_duration.as_secs_f64() > 0.0 {
        (total_bytes as f64 / (1024.0 * 1024.0)) / total_parse_duration.as_secs_f64()
    } else {
        0.0
    };

    println!("Corpus Benchmark Results:");
    println!("  Files analyzed:        {}", count);
    println!(
        "  Total bytes:           {} bytes ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("  Total parse time:      {:.3?}", total_parse_duration);
    println!("  Throughput:            {:.2} MB/s", throughput_mb_s);
    println!("  Latency (p50):         {:.3?}", p50);
    println!("  Latency (p99):         {:.3?}", p99);
    println!(
        "  Success rate:          {}/{} ({:.2}%)",
        success_count,
        count,
        if count > 0 {
            (success_count as f64 / count as f64) * 100.0
        } else {
            0.0
        }
    );

    if !error_counts.is_empty() {
        println!("\nError breakdown:");
        for (err, err_cnt) in &error_counts {
            println!("  - {}: {}", err, err_cnt);
        }
    }

    Ok(())
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        print_help();
        return Err("No subcommand provided. Use 'oxpdf --help' for usage.".into());
    }

    match args[1].as_str() {
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        "-V" | "--version" | "version" => {
            println!("oxpdf 1.0.1");
            Ok(())
        }
        "inspect" => {
            if args.len() < 3 {
                return Err("Usage: oxpdf inspect <file.pdf>".into());
            }
            cmd_inspect(&args[2])
        }
        "extract-text" => {
            let mut file = None;
            let mut page = None;
            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--page" {
                    if i + 1 >= args.len() {
                        return Err("--page requires a page number argument".into());
                    }
                    let p: usize = args[i + 1].parse().map_err(|_| "Invalid page number")?;
                    page = Some(p);
                    i += 2;
                } else if let Some(p_str) = arg.strip_prefix("--page=") {
                    let p: usize = p_str.parse().map_err(|_| "Invalid page number")?;
                    page = Some(p);
                    i += 1;
                } else if arg == "-h" || arg == "--help" {
                    println!("Usage: oxpdf extract-text <file.pdf> [--page <N>]");
                    return Ok(());
                } else if !arg.starts_with('-') && file.is_none() {
                    file = Some(arg.clone());
                    i += 1;
                } else {
                    return Err(format!("Unexpected argument: {}", arg).into());
                }
            }
            let file = file.ok_or("Usage: oxpdf extract-text <file.pdf> [--page <N>]")?;
            cmd_extract_text(&file, page)
        }
        "pack" => {
            if args.len() < 4 {
                return Err("Usage: oxpdf pack <in.pdf> <out.pdf>".into());
            }
            cmd_pack(&args[2], &args[3])
        }
        "bench" => {
            if args.len() < 3 {
                return Err("Usage: oxpdf bench <corpus_dir>".into());
            }
            cmd_bench(&args[2])
        }
        other => {
            Err(format!("Unknown subcommand '{}'. Use 'oxpdf --help' for usage.", other).into())
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err(e) = run(&args) {
        eprintln!("oxpdf error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PDF: &[u8] = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>\nendobj\n\
4 0 obj\n<< /Length 44 >>\nstream\n\
BT\n/F1 12 Tf\n100 700 Td\n(Hello oxpdf CLI) Tj\nET\nendstream\nendobj\n\
xref\n\
0 5\n\
0000000000 65535 f \r\n\
0000000009 00000 n \r\n\
0000000052 00000 n \r\n\
0000000108 00000 n \r\n\
0000000192 00000 n \r\n\
trailer\n\
<< /Size 5 /Root 1 0 R >>\n\
startxref\n\
287\n\
%%EOF";

    fn create_temp_pdf(content: &[u8]) -> (tempfile_helper::TempDir, PathBuf) {
        let temp_dir = tempfile_helper::TempDir::new("oxpdf_cli_test");
        let file_path = temp_dir.path.join("sample.pdf");
        std::fs::write(&file_path, content).unwrap();
        (temp_dir, file_path)
    }

    mod tempfile_helper {
        use std::path::PathBuf;
        pub struct TempDir {
            pub path: PathBuf,
        }
        impl TempDir {
            pub fn new(prefix: &str) -> Self {
                let unique = format!("{}_{}_{}", prefix, std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
                let path = std::env::temp_dir().join(unique);
                std::fs::create_dir_all(&path).unwrap();
                Self { path }
            }
        }
        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }

    #[test]
    fn test_help_and_version() {
        assert!(run(&["oxpdf".into(), "--help".into()]).is_ok());
        assert!(run(&["oxpdf".into(), "-h".into()]).is_ok());
        assert!(run(&["oxpdf".into(), "--version".into()]).is_ok());
        assert!(run(&["oxpdf".into(), "-V".into()]).is_ok());
    }

    #[test]
    fn test_inspect() {
        let (_dir, file_path) = create_temp_pdf(SAMPLE_PDF);
        let res = run(&[
            "oxpdf".into(),
            "inspect".into(),
            file_path.to_str().unwrap().into(),
        ]);
        assert!(res.is_ok());
    }

    #[test]
    fn test_extract_text() {
        let (_dir, file_path) = create_temp_pdf(SAMPLE_PDF);
        let res = run(&[
            "oxpdf".into(),
            "extract-text".into(),
            file_path.to_str().unwrap().into(),
        ]);
        assert!(res.is_ok());

        let res_page = run(&[
            "oxpdf".into(),
            "extract-text".into(),
            file_path.to_str().unwrap().into(),
            "--page".into(),
            "1".into(),
        ]);
        assert!(res_page.is_ok());
    }

    #[test]
    fn test_pack() {
        let (dir, in_path) = create_temp_pdf(SAMPLE_PDF);
        let out_path = dir.path.join("packed.pdf");
        let res = run(&[
            "oxpdf".into(),
            "pack".into(),
            in_path.to_str().unwrap().into(),
            out_path.to_str().unwrap().into(),
        ]);
        assert!(res.is_ok());
        assert!(out_path.exists());

        // Verify packed file can be loaded
        let packed_data = std::fs::read(&out_path).unwrap();
        let doc = oxpdf::Document::load(&packed_data);
        assert!(doc.is_ok());
    }

    #[test]
    fn test_bench() {
        let (dir, _file_path) = create_temp_pdf(SAMPLE_PDF);
        let res = run(&[
            "oxpdf".into(),
            "bench".into(),
            dir.path.to_str().unwrap().into(),
        ]);
        assert!(res.is_ok());
    }
}

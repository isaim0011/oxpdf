use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxpdf::Document as OxDocument;
use std::fs;
use std::path::{Path, PathBuf};

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

fn bench_representative_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparative_parse");

    // Sample 1: Small PDF (15 KB)
    let small_path = Path::new("corpus/pdfjs/calgray.pdf");
    // Sample 2: Medium PDF (105 KB)
    let medium_path = Path::new("corpus/pdfjs/basicapi.pdf");

    // Sample 3: Largest PDF in corpus (~10.87 MB)
    let mut all_files = Vec::new();
    collect_pdfs("corpus", &mut all_files);
    let mut largest_file = PathBuf::new();
    let mut max_len = 0u64;
    for f in &all_files {
        if let Ok(meta) = fs::metadata(f) {
            if meta.len() > max_len {
                max_len = meta.len();
                largest_file = f.clone();
            }
        }
    }

    let samples: Vec<(&str, PathBuf)> = vec![
        ("small_15kb", small_path.to_path_buf()),
        ("medium_105kb", medium_path.to_path_buf()),
        ("large_10mb", largest_file),
    ];

    for (name, path) in samples {
        if !path.exists() {
            continue;
        }
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        group.throughput(Throughput::Bytes(data.len() as u64));

        // oxpdf benchmark
        group.bench_with_input(BenchmarkId::new("oxpdf", name), &data, |b, bytes| {
            b.iter(|| {
                let doc = OxDocument::load(black_box(bytes)).unwrap();
                let count = doc.object_count();
                black_box(count);
            });
        });

        // lopdf benchmark
        group.bench_with_input(BenchmarkId::new("lopdf", name), &data, |b, bytes| {
            b.iter(|| {
                let doc = lopdf::Document::load_mem(black_box(bytes)).unwrap();
                let count = doc.objects.len();
                black_box(count);
            });
        });
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = bench_representative_files
);
criterion_main!(benches);

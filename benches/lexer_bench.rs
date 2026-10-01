use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use oxpdf::lexer::Lexer;
use oxpdf::parser::Parser;

fn benchmark_lexer_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer");

    // Synthetic PDF snippet with mixed tokens: dictionaries, arrays, numbers, strings
    let sample = b"<< /Type /Pages /Count 3 /Kids [ 3 0 R 4 0 R 5 0 R ] /MediaBox [ 0 0 612 792 ] >> \
                   << /Length 42 /Filter /FlateDecode >> stream \
                   q 1 0 0 1 50 700 cm BT /F1 12 Tf (Hello World, oxpdf streaming monster test!) Tj ET Q \
                   endstream";

    group.throughput(Throughput::Bytes(sample.len() as u64));
    group.bench_function("tokenize_mixed_tokens", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sample));
            while let Ok(Some(tok)) = lexer.next_token() {
                black_box(tok);
            }
        });
    });
    group.finish();
}

fn benchmark_parser_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    let sample = b"<< /Type /Catalog /Pages 2 0 R /Outlines 3 0 R /Metadata [1 2 3 4 5] >>";

    group.throughput(Throughput::Bytes(sample.len() as u64));
    group.bench_function("parse_nested_dict", |b| {
        b.iter(|| {
            let mut parser = Parser::new(black_box(sample));
            let obj = parser.parse_object().unwrap();
            black_box(obj);
        });
    });
    group.finish();
}

fn benchmark_simd_structural_scanning(c: &mut Criterion) {
    let mut group = c.benchmark_group("structural_scan");

    // Generate a realistic 16 KiB buffer of PDF stream data with mixed whitespace,
    // delimiters, object keywords, names, and operators.
    let base_pattern = b"  /Type /Pages  /Count 10  /Kids [ 1 0 R 2 0 R 3 0 R ]  /MediaBox [ 0 0 612 792 ] \r\n\
                         << /Length 256 /Filter /FlateDecode >> stream \r\n\
                         q 1 0 0 1 50 750 cm BT /F1 12 Tf (Structural SIMD Benchmark Sample) Tj ET Q \r\n\
                         endstream \r\n";
    let mut corpus = Vec::with_capacity(16 * 1024);
    while corpus.len() + base_pattern.len() <= 16 * 1024 {
        corpus.extend_from_slice(base_pattern);
    }

    group.throughput(Throughput::Bytes(corpus.len() as u64));

    // SIMD / SWAR find_non_whitespace
    group.bench_function("simd_find_non_whitespace_16kb", |b| {
        b.iter(|| {
            let mut offset = 0;
            let slice = black_box(&corpus[..]);
            while offset < slice.len() {
                let skipped = oxpdf::simd::find_non_whitespace(&slice[offset..]);
                offset += skipped.max(1);
            }
        });
    });

    // Scalar fallback find_non_whitespace
    group.bench_function("scalar_find_non_whitespace_16kb", |b| {
        b.iter(|| {
            let mut offset = 0;
            let slice = black_box(&corpus[..]);
            while offset < slice.len() {
                let skipped = oxpdf::simd::scalar_find_non_whitespace(&slice[offset..]);
                offset += skipped.max(1);
            }
        });
    });

    // SIMD / SWAR find_delimiter_or_whitespace
    group.bench_function("simd_find_delim_or_ws_16kb", |b| {
        b.iter(|| {
            let mut offset = 0;
            let slice = black_box(&corpus[..]);
            while offset < slice.len() {
                let advanced = oxpdf::simd::find_delimiter_or_whitespace(&slice[offset..]);
                offset += advanced.max(1);
            }
        });
    });

    // Scalar fallback find_delimiter_or_whitespace
    group.bench_function("scalar_find_delim_or_ws_16kb", |b| {
        b.iter(|| {
            let mut offset = 0;
            let slice = black_box(&corpus[..]);
            while offset < slice.len() {
                let advanced = oxpdf::simd::scalar_find_delimiter_or_whitespace(&slice[offset..]);
                offset += advanced.max(1);
            }
        });
    });

    // 32-byte chunk classification raw throughput
    let chunk: [u8; 32] = corpus[..32].try_into().unwrap();
    group.throughput(Throughput::Bytes(32));
    group.bench_function("simd_classify_chunk_32", |b| {
        b.iter(|| {
            let res = oxpdf::simd::classify_chunk_32(black_box(&chunk));
            black_box(res);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_lexer_throughput,
    benchmark_parser_throughput,
    benchmark_simd_structural_scanning
);
criterion_main!(benches);

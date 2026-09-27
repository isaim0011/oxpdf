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

criterion_group!(
    benches,
    benchmark_lexer_throughput,
    benchmark_parser_throughput
);
criterion_main!(benches);

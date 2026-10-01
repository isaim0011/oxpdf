use oxpdf_fuzz::{fuzz_filter, fuzz_lexer, fuzz_parser, fuzz_xref};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    println!("========================================================");
    println!("oxpdf Continuous Fuzzing Verification Harness");
    println!("========================================================");

    let start = Instant::now();

    // 1. Verify Lexer Target
    print!("[1/4] Verifying 'lexer' fuzz target... ");
    let lexer_samples: Vec<&[u8]> = vec![
        b"",
        b"   \r\n\t  ",
        b"% standard comment\r\n",
        b"% unterminated comment without newline",
        b"/Name /Another#20Name /#41#42#43 / /#",
        b"123 -456 +789 0 00001",
        b"123.456 -.789 +0.001 . 123.45.67",
        b"true false null",
        b"(simple string) (escaped \\n \\r \\t \\( \\) \\\\ string)",
        b"(unterminated string",
        b"(nested (parentheses (string)) in pdf)",
        b"<48656C6C6F20576F726C64>",
        b"<unterminated hex string",
        b"<odd hex digits 123>",
        b"<< /Key /Value /Array [ 1 2 3 ] /Dict << /Inner 42 >> >>",
        b"<< << << << >> >> >> >>",
        b"[ [ [ [ ] ] ] ]",
        b">>>> <<<< ]]]] [[[[",
        b"stream\r\n12345\r\nendstream",
        b"stream\nraw data without endstream",
        &[0x00, 0xFF, 0xFE, 0xAA, 0x55, 0x7F, 0x80, 0x01],
    ];
    for sample in &lexer_samples {
        fuzz_lexer(sample);
    }
    // Pseudo-random noise
    let mut rand_buf = vec![0u8; 8192];
    for (i, b) in rand_buf.iter_mut().enumerate() {
        *b = ((i * 104729 + 17) % 256) as u8;
    }
    fuzz_lexer(&rand_buf);
    println!("OK ({} samples + 8KB noise)", lexer_samples.len());

    // 2. Verify Parser Target
    print!("[2/4] Verifying 'parser' fuzz target... ");
    let deep_array = b"[".repeat(300);
    fuzz_parser(&deep_array);

    let mut deep_dict = Vec::new();
    for _ in 0..300 {
        deep_dict.extend_from_slice(b"<< /K ");
    }
    fuzz_parser(&deep_dict);

    let parser_samples: Vec<&[u8]> = vec![
        b"",
        b"null",
        b"true false",
        b"12345",
        b"1 0 R",
        b"99999999999999999999999999 0 R",
        b"-1 -1 R",
        b"/TestName",
        b"(Unclosed string literal",
        b"<Malformed hex>",
        b"<< /A 1 /B 2 /C [ 3 4 << /D 5 >> ] >>",
        b"1 0 obj << /Type /Catalog >> endobj",
        b"1 0 obj << /Length 10 >> stream\r\nabcdefghij\r\nendstream endobj",
        b"1 0 obj stream without length or dictionary endstream",
    ];
    for sample in &parser_samples {
        fuzz_parser(sample);
    }
    println!("OK ({} samples + recursion boundary tests)", parser_samples.len());

    // 3. Verify XRef Target
    print!("[3/4] Verifying 'xref' fuzz target... ");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut xref_samples_count = 0;

    let seed_path = manifest_dir.join("corpus").join("xref").join("seed.pdf");
    if let Ok(data) = fs::read(&seed_path) {
        fuzz_xref(&data);
        xref_samples_count += 1;
    }

    let minimal_pdf_path = manifest_dir.parent().unwrap().join("corpus").join("minimal.pdf");
    if let Ok(data) = fs::read(&minimal_pdf_path) {
        fuzz_xref(&data);
        xref_samples_count += 1;
    }

    let xref_adversarial: Vec<&[u8]> = vec![
        b"",
        b"%PDF-1.4\n",
        b"%PDF-1.7\n% binary header \xE2\xE3\xCF\xD3\n",
        b"garbage before %PDF-1.4\n1 0 obj << /Type /Catalog >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF",
        b"%PDF-1.4\nxref\n0 1\n0000000000 65535 f \ntrailer\n<< /Size 1 >>\nstartxref\n9\n%%EOF",
        b"%PDF-1.4\nstartxref\n999999999\n%%EOF",
        b"%PDF-1.4\nstartxref\n0\n%%EOF",
        b"%PDF-1.4\ntrailer << /Encrypt << /Filter /Standard >> >>\nstartxref\n10\n%%EOF",
        b"%PDF-1.5\n1 0 obj << /Type /XRef /Size 2 /W [1 2 1] >> stream\n\x01\x00\x0a\x00endstream\nendobj\nstartxref\n9\n%%EOF",
    ];
    for sample in &xref_adversarial {
        fuzz_xref(sample);
        xref_samples_count += 1;
    }

    let mut random_bytes = vec![0u8; 16384];
    for (i, b) in random_bytes.iter_mut().enumerate() {
        *b = ((i * 31337 + 101) % 256) as u8;
    }
    fuzz_xref(&random_bytes);
    println!("OK ({} samples + 16KB random stream)", xref_samples_count);

    // 4. Verify Filter Target
    print!("[4/4] Verifying 'filter' fuzz target... ");
    let filter_samples: Vec<&[u8]> = vec![
        b"",
        b"48656c6c6f20576f726c64>",
        b"4 8 6 5 6 c 6 c 6 f >",
        b"48656c6c6f2>",
        b"invalid hex non-ascii \xFF\xFE>",
        b"<~87cURD]i,\"Ebo80~>",
        b"<~z~>",
        b"<~zzzz~>",
        b"<~87cURD]i,\"Ebo80",
        b"invalid ascii85 characters !@#$%^&*()",
        &[0x78, 0x9c, 0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00, 0x18, 0xab, 0x04, 0x3d],
        &[0x78, 0x9c],
        &[0x78, 0x9c, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF],
    ];
    for sample in &filter_samples {
        fuzz_filter(sample);
    }
    println!("OK ({} samples)", filter_samples.len());

    println!("--------------------------------------------------------");
    println!("All 4 fuzz targets successfully verified in {:.2}ms with 0 panics!", start.elapsed().as_secs_f64() * 1000.0);
    println!("Continuous Fuzzing Infrastructure is READY.");
    println!("========================================================");
}

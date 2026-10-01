use oxpdf::Serializer;
use std::fs::File;
use std::io::{BufWriter, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let size_gb: usize = if args.len() > 1 {
        args[1].parse().unwrap_or(2)
    } else {
        2
    };
    let target_path = if args.len() > 2 {
        args[2].clone()
    } else {
        format!("synthetic_{}gb.pdf", size_gb)
    };

    println!(
        "Generating synthetic {} GB PDF at `{}`...",
        size_gb, target_path
    );

    let file = File::create(&target_path)?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);
    let mut ser = Serializer::new(&mut writer);

    ser.write_header((1, 4))?;

    let mut offsets = Vec::new();

    // Catalog & Pages
    let cat_offset = ser.write_indirect_object_header(1, 0)?;
    ser.write_bytes(b"<< /Type /Catalog /Pages 2 0 R >>")?;
    ser.write_indirect_object_footer()?;
    offsets.push((1, cat_offset));

    let pages_offset = ser.write_indirect_object_header(2, 0)?;
    ser.write_bytes(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    ser.write_indirect_object_footer()?;
    offsets.push((2, pages_offset));

    let page_offset = ser.write_indirect_object_header(3, 0)?;
    ser.write_bytes(b"<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>")?;
    ser.write_indirect_object_footer()?;
    offsets.push((3, page_offset));

    // Calculate how many 1MB chunks needed to reach target_gb
    let target_bytes = size_gb * 1024 * 1024 * 1024;
    let chunk_size = 1024 * 1024; // 1MB chunk
    let chunks = target_bytes / chunk_size;

    let chunk_data = vec![b'A'; chunk_size];

    let stream_offset = ser.write_indirect_object_header(4, 0)?;
    ser.write_bytes(format!("<< /Length {} >>\nstream\n", target_bytes).as_bytes())?;
    for _ in 0..chunks {
        ser.write_bytes(&chunk_data)?;
    }
    ser.write_bytes(b"\nendstream")?;
    ser.write_indirect_object_footer()?;
    offsets.push((4, stream_offset));

    // Write xref table
    let xref_offset = ser.bytes_written();
    ser.write_bytes(format!("xref\n0 {}\n", offsets.len() + 1).as_bytes())?;
    ser.write_bytes(b"0000000000 65535 f \r\n")?;
    for (_, off) in &offsets {
        ser.write_bytes(format!("{:010} 00000 n \r\n", off).as_bytes())?;
    }

    ser.write_bytes(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF",
            offsets.len() + 1,
            xref_offset
        )
        .as_bytes(),
    )?;

    writer.flush()?;
    println!("Successfully generated synthetic {} GB PDF.", size_gb);
    Ok(())
}

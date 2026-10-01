use oxpdf::{Document, MmapSource, PdfSource};
use std::fs::File;

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
    println!("Mmap created successfully. File size: {:.2} MB", source.len() as f64 / (1024.0 * 1024.0));

    let slice = source.as_slice()?;
    let doc = Document::load(slice)?;

    println!("Document loaded successfully.");
    println!("Object count: {}", doc.object_count());
    if let Ok(count) = doc.page_count() {
        println!("Page count: {}", count);
    }

    // Access individual objects to verify zero-copy lookup
    for id in 1..=doc.object_count().min(10) as u32 {
        if let Ok(Some(obj)) = doc.get_object(id) {
            println!("  Object {}: {:?}", id, obj);
        }
    }

    println!("Memory measurement run completed successfully.");
    Ok(())
}

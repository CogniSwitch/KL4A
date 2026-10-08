//! Dump this port's view of a PDF, for differential comparison against
//! pdfplumber. Pairs with `fixtures/harness/dump_pdf.py`, which prints the same
//! JSON shape from the real pdfplumber -- diffing the two is how the extraction
//! layer is validated.
//!
//!     cargo run -p sopkb-core --features pdf --example pdf_dump -- FILE.pdf [--chars]

use sopkb_core::pdf::{content, words};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: pdf_dump FILE.pdf [--chars]");
        std::process::exit(2);
    };
    let want_chars = args.iter().any(|a| a == "--chars");

    let doc = lopdf::Document::load(path).expect("load pdf");
    let pages = content::extract_pages(&doc);

    println!("{{\"pages\": [");
    for (i, page) in pages.iter().enumerate() {
        if i > 0 {
            println!(",");
        }
        println!("  {{");
        println!("    \"index\": {},", i + 1);
        println!("    \"width\": {}, \"height\": {},", page.width, page.height);
        println!("    \"n_chars\": {},", page.chars.len());
        if want_chars {
            println!("    \"chars\": [");
            for (j, c) in page.chars.iter().enumerate() {
                let comma = if j + 1 < page.chars.len() { "," } else { "" };
                println!(
                    "      {{\"text\": {}, \"x0\": {:.4}, \"x1\": {:.4}, \"top\": {:.4}, \"bottom\": {:.4}, \"size\": {:.4}, \"upright\": {}, \"fontname\": {}}}{}",
                    json_str(&c.text),
                    c.x0,
                    c.x1,
                    c.top,
                    c.bottom,
                    c.size,
                    c.upright,
                    json_str(&c.fontname),
                    comma
                );
            }
            println!("    ],");
        }
        let text = words::extract_text(&page.chars, &words::WordExtractor::default());
        println!("    \"extract_text\": {}", json_str(&text));
        print!("  }}");
    }
    println!("\n]}}");
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

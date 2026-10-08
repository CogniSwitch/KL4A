//! Print `normalize_pdf`'s output for one PDF, verbatim, for differential
//! comparison against the Python. Pairs with
//! `fixtures/harness/diff_normalize_pdf.py`.
//!
//!     cargo run -p sopkb-core --features pdf --example pdf_normalize -- FILE.pdf

use std::io::Write;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: pdf_normalize FILE.pdf");
        std::process::exit(2);
    };
    match sopkb_core::pdf::normalize_pdf(std::path::Path::new(&path)) {
        // Write the bytes directly: `println!` would append a newline the real
        // return value does not have, and the comparison is byte-exact.
        Ok(text) => std::io::stdout().write_all(text.as_bytes()).expect("write"),
        Err(msg) => {
            print!("<<RAISED>> {msg}");
            std::process::exit(0);
        }
    }
}

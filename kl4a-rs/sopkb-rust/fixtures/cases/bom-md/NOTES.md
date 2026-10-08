expected-python/ pins today's (unfixed) BOM-loses-the-heading behavior per P-N5. Once the
Rust side strips a leading BOM (the recommended fix), this case is expected to diff against
expected-python/ by design; the fixed behavior belongs in a separately-committed
`expected-rust-fixed/` snapshot, not by editing expected-python/.

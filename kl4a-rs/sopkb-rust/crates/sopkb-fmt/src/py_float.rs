//! Emulates Python's `repr(float)` shortest-round-trip formatting, which is what both
//! `json.dumps` and f-strings use to stringify a float. The one divergence from Rust's
//! own `f64::to_string()` that matters for this codebase: Python always shows a decimal
//! point (`1.0`, never `1`), so a Rust shortest-round-trip result with no `.`/`e` needs
//! `.0` appended.

pub fn py_float(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
    }
    let s = f.to_string();
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{s}.0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_numbers_get_decimal_point() {
        assert_eq!(py_float(1.0), "1.0");
        assert_eq!(py_float(0.0), "0.0");
        assert_eq!(py_float(-2.0), "-2.0");
    }

    #[test]
    fn fractional_values_round_trip() {
        assert_eq!(py_float(0.82), "0.82");
        assert_eq!(py_float(0.95), "0.95");
        assert_eq!(py_float(1.5), "1.5");
    }
}

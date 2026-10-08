//! Python string-method emulations whose exact quirks other code depends on.

/// Emulates Python's `str.title()`: uppercases the first "cased" character of every
/// maximal run of cased characters and lowercases the rest; anything not cased
/// (digits, punctuation, apostrophes, whitespace, CJK ideographs) is left untouched
/// and acts as a word boundary. "Cased" here means Unicode Uppercase or Lowercase
/// property, matching `char::is_uppercase()`/`is_lowercase()`.
///
/// Quirks this must reproduce (docs/port/port-mapping-a-core-data.md §3.3
/// `create_bundle`): `"demo-bundle"` -> `"Demo Bundle"`, `"glp1-healthcare"` ->
/// `"Glp1 Healthcare"`, `"acme's-sop"` -> `"Acme'S Sop"` (apostrophe splits the
/// word), `"v1.2-notes"` -> `"V1.2 Notes"` (digits are boundaries, not cased).
pub fn py_title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_cased = false;
    for ch in s.chars() {
        let is_cased = ch.is_uppercase() || ch.is_lowercase();
        if is_cased {
            if prev_cased {
                out.extend(ch.to_lowercase());
            } else {
                out.extend(ch.to_uppercase());
            }
        } else {
            out.push(ch);
        }
        prev_cased = is_cased;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_create_bundle_examples() {
        assert_eq!(py_title_case("demo bundle"), "Demo Bundle");
        assert_eq!(py_title_case("glp1 healthcare"), "Glp1 Healthcare");
        assert_eq!(py_title_case("acme's sop"), "Acme'S Sop");
        assert_eq!(py_title_case("v1.2 notes"), "V1.2 Notes");
    }
}

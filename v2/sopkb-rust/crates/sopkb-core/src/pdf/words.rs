//! Faithful ports of the pdfplumber text-layer algorithms that
//! `tools/sopkb/sopkb/normalize.py` depends on, transcribed from pdfplumber
//! 0.11.10's own `utils/text.py` and `utils/clustering.py` rather than
//! reimplemented from behavior.
//!
//! Per CATCHUP_PLAN.md D2 the whole point is byte-fidelity to *this* specific
//! algorithm -- so where pdfplumber does something surprising (single-linkage
//! chaining in `cluster_list`, measuring interline distance top-to-top rather
//! than between bounding boxes, expanding ligatures only at word-merge time)
//! that surprise is reproduced deliberately. Do not "clean these up".

use std::collections::HashMap;

/// pdfplumber `DEFAULT_X_TOLERANCE`.
pub const DEFAULT_X_TOLERANCE: f64 = 3.0;
/// pdfplumber `DEFAULT_Y_TOLERANCE`.
pub const DEFAULT_Y_TOLERANCE: f64 = 3.0;

/// One pdfplumber `char` dict, restricted to the keys this port actually reads.
///
/// Field names deliberately mirror pdfplumber's dict keys so the ported code
/// below reads the same as the Python it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfChar {
    pub text: String,
    pub x0: f64,
    pub x1: f64,
    pub top: f64,
    pub bottom: f64,
    pub doctop: f64,
    pub upright: bool,
    pub size: f64,
    pub fontname: String,
}

/// One pdfplumber `word` dict, restricted to the keys this port actually reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfWord {
    pub text: String,
    pub x0: f64,
    pub x1: f64,
    pub top: f64,
    pub bottom: f64,
    pub doctop: f64,
    pub upright: bool,
}

/// pdfplumber's `T_dir`. Only the four values `validate_directions` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Ttb,
    Btt,
    Ltr,
    Rtl,
}

/// pdfplumber `LIGATURES`. Applied in `merge_chars`, i.e. only when characters
/// are combined into a word -- a raw `page.chars` entry keeps the ligature.
fn expand_ligature(text: &str) -> &str {
    match text {
        "\u{FB00}" => "ff",
        "\u{FB03}" => "ffi",
        "\u{FB04}" => "ffl",
        "\u{FB01}" => "fi",
        "\u{FB02}" => "fl",
        "\u{FB06}" => "st",
        "\u{FB05}" => "st",
        other => other,
    }
}

// ---------------------------------------------------------------------------
// utils/clustering.py
// ---------------------------------------------------------------------------

/// Total order over f64 for sorting, with -0.0 normalized to 0.0 so it hashes
/// and compares equal to 0.0 the way Python's float does.
fn norm(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else {
        x
    }
}

fn key_bits(x: f64) -> u64 {
    norm(x).to_bits()
}

fn sort_f64(xs: &mut [f64]) {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
}

/// `cluster_list`: single-linkage chaining over the *sorted* values -- `last`
/// advances to each value, not to the cluster's first element, so a long run of
/// closely-spaced values all collapses into one cluster even when its extremes
/// are far more than `tolerance` apart.
fn cluster_list(xs: &[f64], tolerance: f64) -> Vec<Vec<f64>> {
    let mut sorted: Vec<f64> = xs.to_vec();
    sort_f64(&mut sorted);
    if tolerance == 0.0 || sorted.len() < 2 {
        return sorted.into_iter().map(|x| vec![x]).collect();
    }
    let mut groups: Vec<Vec<f64>> = Vec::new();
    let mut current = vec![sorted[0]];
    let mut last = sorted[0];
    for &x in &sorted[1..] {
        if x <= last + tolerance {
            current.push(x);
        } else {
            groups.push(std::mem::replace(&mut current, vec![x]));
        }
        last = x;
    }
    groups.push(current);
    groups
}

/// `make_cluster_dict`: note the `set(values)` -- duplicate values are collapsed
/// before clustering, so repeated coordinates never widen a cluster by chaining.
fn make_cluster_dict(values: impl Iterator<Item = f64>, tolerance: f64) -> HashMap<u64, usize> {
    let mut seen: Vec<f64> = Vec::new();
    let mut seen_bits: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for v in values {
        if seen_bits.insert(key_bits(v)) {
            seen.push(v);
        }
    }
    let clusters = cluster_list(&seen, tolerance);
    let mut out = HashMap::new();
    for (i, cluster) in clusters.iter().enumerate() {
        for &val in cluster {
            out.insert(key_bits(val), i);
        }
    }
    out
}

/// `cluster_objects(..., preserve_order=False)`: a *stable* sort by cluster
/// index, then `groupby`. Stability is what keeps each cluster's members in
/// their original relative order, which several callers rely on.
pub fn cluster_objects<T, F>(xs: &[T], key_fn: F, tolerance: f64) -> Vec<Vec<&T>>
where
    F: Fn(&T) -> f64,
{
    let cluster_dict = make_cluster_dict(xs.iter().map(&key_fn), tolerance);
    let mut tagged: Vec<(usize, &T)> = xs
        .iter()
        .map(|x| (*cluster_dict.get(&key_bits(key_fn(x))).unwrap_or(&0), x))
        .collect();
    tagged.sort_by_key(|(i, _)| *i); // Rust's sort_by_key is stable, like Python's sorted
    let mut out: Vec<Vec<&T>> = Vec::new();
    let mut current_key: Option<usize> = None;
    for (i, x) in tagged {
        if current_key != Some(i) {
            out.push(Vec::new());
            current_key = Some(i);
        }
        out.last_mut().unwrap().push(x);
    }
    out
}

// ---------------------------------------------------------------------------
// utils/text.py -- WordExtractor
// ---------------------------------------------------------------------------

fn line_cluster_key(line_dir: Dir, c: &PdfChar) -> f64 {
    match line_dir {
        Dir::Ttb => c.top,
        Dir::Btt => -c.bottom,
        Dir::Ltr => c.x0,
        Dir::Rtl => -c.x1,
    }
}

fn char_sort_key(char_dir: Dir, c: &PdfChar) -> (f64, f64) {
    match char_dir {
        Dir::Ttb => (c.top, c.bottom),
        Dir::Btt => (-(c.bottom), -c.top),
        Dir::Ltr => (c.x0, c.x0),
        Dir::Rtl => (-c.x1, -c.x0),
    }
}

/// Options for `WordExtractor`, restricted to what `normalize.py` actually sets.
/// Defaults match pdfplumber's own (`line_dir="ttb"`, `char_dir="ltr"`, so
/// `line_dir_rotated="ltr"` and `char_dir_rotated="ttb"`).
#[derive(Debug, Clone, Copy)]
pub struct WordExtractor {
    pub x_tolerance: f64,
    pub y_tolerance: f64,
    pub keep_blank_chars: bool,
    pub use_text_flow: bool,
}

impl Default for WordExtractor {
    fn default() -> Self {
        Self {
            x_tolerance: DEFAULT_X_TOLERANCE,
            y_tolerance: DEFAULT_Y_TOLERANCE,
            keep_blank_chars: false,
            use_text_flow: false,
        }
    }
}

impl WordExtractor {
    const LINE_DIR: Dir = Dir::Ttb;
    const CHAR_DIR: Dir = Dir::Ltr;
    /// `line_dir_rotated = line_dir_rotated or char_dir` -> "ltr".
    const LINE_DIR_ROTATED: Dir = Dir::Ltr;
    /// `char_dir_rotated = char_dir_rotated or line_dir` -> "ttb".
    const CHAR_DIR_ROTATED: Dir = Dir::Ttb;

    /// `get_char_dir`, with `vertical_ttb`/`horizontal_ltr` left at their
    /// (deprecated but default) `True`, so only the final branch can be taken.
    fn get_char_dir(upright: bool) -> Dir {
        if upright {
            Self::CHAR_DIR
        } else {
            Self::CHAR_DIR_ROTATED
        }
    }

    /// `char_begins_new_word`. The intraline test measures from the *end* of the
    /// previous char to the *start* of the current one; the interline test
    /// measures top-to-top (not between boxes) precisely because successive
    /// lines' boxes often overlap slightly.
    fn char_begins_new_word(
        prev: &PdfChar,
        curr: &PdfChar,
        direction: Dir,
        x_tolerance: f64,
        y_tolerance: f64,
    ) -> bool {
        let (x, y, ay, cy, ax, bx, cx) = match direction {
            Dir::Ltr => (x_tolerance, y_tolerance, prev.top, curr.top, prev.x0, prev.x1, curr.x0),
            Dir::Rtl => (x_tolerance, y_tolerance, prev.top, curr.top, -prev.x1, -prev.x0, -curr.x1),
            Dir::Ttb => (y_tolerance, x_tolerance, prev.x0, curr.x0, prev.top, prev.bottom, curr.top),
            Dir::Btt => (y_tolerance, x_tolerance, prev.x0, curr.x0, -prev.bottom, -prev.top, -curr.bottom),
        };
        (cx < ax) || (cx > bx + x) || ((cy - ay).abs() > y)
    }

    /// `iter_chars_to_words`. `split_at_punctuation` is always falsy here, so
    /// that branch is omitted.
    fn iter_chars_to_words<'a>(&self, ordered: &[&'a PdfChar], direction: Dir) -> Vec<Vec<&'a PdfChar>> {
        let mut out: Vec<Vec<&'a PdfChar>> = Vec::new();
        let mut current: Vec<&'a PdfChar> = Vec::new();
        for &ch in ordered {
            let is_space = !ch.text.is_empty() && ch.text.chars().all(char::is_whitespace);
            if !self.keep_blank_chars && is_space {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            } else if !current.is_empty()
                && Self::char_begins_new_word(
                    current.last().unwrap(),
                    ch,
                    direction,
                    self.x_tolerance,
                    self.y_tolerance,
                )
            {
                out.push(std::mem::replace(&mut current, vec![ch]));
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
        out
    }

    /// `iter_chars_to_lines`.
    fn iter_chars_to_lines<'a>(&self, chars: &[&'a PdfChar]) -> Vec<(Vec<&'a PdfChar>, Dir)> {
        let upright = chars[0].upright;
        let line_dir = if upright { Self::LINE_DIR } else { Self::LINE_DIR_ROTATED };
        let char_dir = Self::get_char_dir(upright);
        let tolerance = if matches!(line_dir, Dir::Ttb | Dir::Btt) {
            self.y_tolerance
        } else {
            self.x_tolerance
        };
        let owned: Vec<&'a PdfChar> = chars.to_vec();
        let subclusters = cluster_objects(&owned, |c| line_cluster_key(line_dir, c), tolerance);
        subclusters
            .into_iter()
            .map(|sc| {
                let mut sorted: Vec<&'a PdfChar> = sc.into_iter().copied().collect();
                sorted.sort_by(|a, b| {
                    let ka = char_sort_key(char_dir, a);
                    let kb = char_sort_key(char_dir, b);
                    ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
                });
                (sorted, char_dir)
            })
            .collect()
    }

    /// `merge_chars`.
    fn merge_chars(ordered: &[&PdfChar]) -> PdfWord {
        let x0 = ordered.iter().map(|c| c.x0).fold(f64::INFINITY, f64::min);
        let x1 = ordered.iter().map(|c| c.x1).fold(f64::NEG_INFINITY, f64::max);
        let top = ordered.iter().map(|c| c.top).fold(f64::INFINITY, f64::min);
        let bottom = ordered.iter().map(|c| c.bottom).fold(f64::NEG_INFINITY, f64::max);
        let doctop_adj = ordered[0].doctop - ordered[0].top;
        PdfWord {
            text: ordered.iter().map(|c| expand_ligature(&c.text)).collect::<String>(),
            x0,
            x1,
            top,
            bottom,
            doctop: top + doctop_adj,
            upright: ordered[0].upright,
        }
    }

    /// `iter_extract_tuples`: note `itertools.groupby` groups *consecutive*
    /// runs sharing `upright`, so it is a run-length grouping over the incoming
    /// char order, not a global partition into upright/rotated.
    pub fn extract_words(&self, chars: &[PdfChar]) -> Vec<PdfWord> {
        let mut out = Vec::new();
        for group in group_consecutive_by_upright(chars) {
            let line_groups: Vec<(Vec<&PdfChar>, Dir)> = if self.use_text_flow {
                vec![(group.clone(), Self::CHAR_DIR)]
            } else {
                self.iter_chars_to_lines(&group)
            };
            for (line_chars, direction) in line_groups {
                for word_chars in self.iter_chars_to_words(&line_chars, direction) {
                    out.push(Self::merge_chars(&word_chars));
                }
            }
        }
        out
    }
}

fn group_consecutive_by_upright(chars: &[PdfChar]) -> Vec<Vec<&PdfChar>> {
    let mut out: Vec<Vec<&PdfChar>> = Vec::new();
    let mut last: Option<bool> = None;
    for c in chars {
        if last != Some(c.upright) {
            out.push(Vec::new());
            last = Some(c.upright);
        }
        out.last_mut().unwrap().push(c);
    }
    out
}

/// `utils.text.extract_text(chars)` with `layout=False` (pdfplumber's default,
/// and the only mode `normalize.py` uses). Words are re-clustered into lines by
/// `top` *after* word extraction, which is why a word's line membership here can
/// differ from the line it was built in.
///
/// The final `TextMap(...).as_string` step is an identity for the default
/// `ltr`/`ttb` render directions (see `TextMap.to_string`'s first branch), so it
/// is omitted rather than ported.
pub fn extract_text(chars: &[PdfChar], extractor: &WordExtractor) -> String {
    if chars.is_empty() {
        return String::new();
    }
    let words = extractor.extract_words(chars);
    let lines = cluster_objects(&words, |w| w.top, extractor.y_tolerance);
    lines
        .iter()
        .map(|line| line.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// utils/text.py -- dedupe_chars
// ---------------------------------------------------------------------------

/// Python's `round(x, ndigits)`: correctly-rounded, round-half-to-even against
/// the double's *exact* binary value. Load-bearing for
/// `dedupe_chars_size_tolerant`'s size bucketing.
///
/// Deliberately routed through decimal formatting rather than the obvious
/// `(x * 10^n).round_ties_even() / 10^n`: that scale-then-round form disagrees
/// with Python wherever the scaling itself introduces error. `round(0.35, 1)`
/// is the canonical case -- 0.35 is really 0.34999999999999997780, so Python
/// yields 0.3, but `0.35 * 10.0` evaluates to 3.5000000000000004, which rounds
/// *up* to 0.4. Rust's float formatter, like Python's `repr`, rounds the exact
/// value, so the two agree.
pub fn python_round(x: f64, ndigits: u32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{:.*}", ndigits as usize, x).parse::<f64>().unwrap_or(x)
}

/// `pdfplumber.utils.text.dedupe_chars(chars, tolerance)` with the default
/// `extra_attrs=("fontname", "size")`, generalized over the grouping key so
/// `normalize.py`'s size-tolerant wrapper can supply a rounded size.
///
/// Returns the surviving chars' indices into `chars`, in original order --
/// modelling Python's `sorted(deduped, key=chars.index)` without depending on
/// dict equality.
fn dedupe_chars_by<F>(chars: &[PdfChar], tolerance: f64, group_key: F) -> Vec<usize>
where
    F: Fn(&PdfChar) -> (bool, String, String, u64),
{
    let mut by_key: HashMap<(bool, String, String, u64), Vec<usize>> = HashMap::new();
    let mut order: Vec<(bool, String, String, u64)> = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        let k = group_key(c);
        if !by_key.contains_key(&k) {
            order.push(k.clone());
        }
        by_key.entry(k).or_default().push(i);
    }

    let mut kept: Vec<usize> = Vec::new();
    for k in order {
        let idxs = &by_key[&k];
        let group: Vec<&PdfChar> = idxs.iter().map(|&i| &chars[i]).collect();
        // Cluster by doctop, then by x0, keeping one representative per cell.
        for y_cluster in cluster_objects(&group, |c| c.doctop, tolerance) {
            let y_owned: Vec<&PdfChar> = y_cluster.into_iter().copied().collect();
            for x_cluster in cluster_objects(&y_owned, |c| c.x0, tolerance) {
                // `sorted(x_cluster, key=itemgetter("doctop", "x0"))[0]`
                let mut members: Vec<&PdfChar> = x_cluster.into_iter().copied().collect();
                members.sort_by(|a, b| {
                    (a.doctop, a.x0).partial_cmp(&(b.doctop, b.x0)).unwrap_or(std::cmp::Ordering::Equal)
                });
                let winner = members[0];
                // Map the winner back to a concrete index within this key group.
                let idx = idxs
                    .iter()
                    .copied()
                    .find(|&i| std::ptr::eq(&chars[i], winner))
                    .expect("winner came from this group");
                kept.push(idx);
            }
        }
    }
    kept.sort_unstable();
    kept
}

/// `normalize.py`'s `_dedupe_chars`: pdfplumber's own dedup, but with `size`
/// rounded to `_DEDUPE_SIZE_DECIMALS` before grouping, so two genuine duplicate
/// draws whose computed sizes differ by a float hair still land in one group.
/// The character actually kept is always the real, unrounded original.
pub fn dedupe_chars_size_tolerant(chars: &[PdfChar], tolerance: f64, size_decimals: u32) -> Vec<usize> {
    dedupe_chars_by(chars, tolerance, |c| {
        (c.upright, c.text.clone(), c.fontname.clone(), key_bits(python_round(c.size, size_decimals)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(text: &str, x0: f64, x1: f64, top: f64, bottom: f64) -> PdfChar {
        PdfChar {
            text: text.to_string(),
            x0,
            x1,
            top,
            bottom,
            doctop: top,
            upright: true,
            size: 10.0,
            fontname: "F1".to_string(),
        }
    }

    #[test]
    fn cluster_list_chains_single_linkage() {
        // 0,2,4,6 each within tolerance 2 of the previous -> ONE cluster spanning
        // 6 points, far more than the tolerance itself. This chaining is
        // pdfplumber's actual behavior and several call sites depend on it.
        let groups = cluster_list(&[0.0, 2.0, 4.0, 6.0], 2.0);
        assert_eq!(groups.len(), 1);
        // A gap strictly greater than tolerance does break the chain.
        let groups = cluster_list(&[0.0, 2.0, 5.0], 2.0);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn cluster_list_zero_tolerance_is_one_per_value() {
        let groups = cluster_list(&[3.0, 1.0, 2.0], 0.0);
        assert_eq!(groups, vec![vec![1.0], vec![2.0], vec![3.0]]);
    }

    #[test]
    fn cluster_objects_is_stable_within_a_cluster() {
        let chars = vec![ch("a", 0.0, 5.0, 10.0, 20.0), ch("b", 5.0, 10.0, 10.0, 20.0)];
        let clusters = cluster_objects(&chars, |c| c.top, 3.0);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0][0].text, "a");
        assert_eq!(clusters[0][1].text, "b");
    }

    #[test]
    fn cluster_objects_orders_clusters_by_value_not_input_order() {
        let chars = vec![ch("low", 0.0, 5.0, 100.0, 110.0), ch("high", 0.0, 5.0, 10.0, 20.0)];
        let clusters = cluster_objects(&chars, |c| c.top, 3.0);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0][0].text, "high", "cluster index follows ascending value");
    }

    #[test]
    fn words_split_on_horizontal_gap_beyond_tolerance() {
        // "ab" then a 4pt gap (> x_tolerance 3) then "c".
        let chars = vec![
            ch("a", 0.0, 5.0, 10.0, 20.0),
            ch("b", 5.0, 10.0, 10.0, 20.0),
            ch("c", 14.0, 19.0, 10.0, 20.0),
        ];
        let words = WordExtractor::default().extract_words(&chars);
        assert_eq!(words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), vec!["ab", "c"]);
    }

    #[test]
    fn words_do_not_split_on_gap_within_tolerance() {
        let chars = vec![
            ch("a", 0.0, 5.0, 10.0, 20.0),
            ch("b", 7.0, 12.0, 10.0, 20.0), // 2pt gap, <= 3
        ];
        let words = WordExtractor::default().extract_words(&chars);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "ab");
    }

    #[test]
    fn word_split_boundary_is_strictly_greater_than_tolerance() {
        // Gap of exactly x_tolerance must NOT split: the test is `cx > bx + x`.
        let chars = vec![ch("a", 0.0, 5.0, 10.0, 20.0), ch("b", 8.0, 13.0, 10.0, 20.0)];
        let words = WordExtractor::default().extract_words(&chars);
        assert_eq!(words.len(), 1, "a gap of exactly 3.0 is not a word break");
    }

    #[test]
    fn backwards_draw_is_reordered_by_the_line_sort_not_split() {
        // Cross-checked against pdfplumber 0.11.10, which yields a single word
        // "ba" here: the per-line sort by `char_sort_key("ltr")` = (x0, x0) runs
        // BEFORE word-building, so for ordinary upright text the `cx < ax` half
        // of the intraline test can never fire. Asserting the real behavior
        // rather than the intuitive one.
        let chars = vec![ch("a", 10.0, 15.0, 10.0, 20.0), ch("b", 9.0, 14.0, 10.0, 20.0)];
        let words = WordExtractor::default().extract_words(&chars);
        assert_eq!(words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), vec!["ba"]);
    }

    #[test]
    fn word_splits_when_current_char_starts_left_of_previous_under_text_flow() {
        // `use_text_flow` skips the line sort, which is the only way to actually
        // reach the `cx < ax` branch. pdfplumber yields ["a", "b"] here.
        let chars = vec![ch("a", 10.0, 15.0, 10.0, 20.0), ch("b", 9.0, 14.0, 10.0, 20.0)];
        let ex = WordExtractor { use_text_flow: true, ..Default::default() };
        assert_eq!(ex.extract_words(&chars).iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn word_splits_on_interline_top_delta_beyond_tolerance() {
        // The interline half: measured top-to-top, not between boxes.
        let chars = vec![ch("a", 0.0, 5.0, 10.0, 20.0), ch("b", 5.0, 10.0, 14.0, 24.0)];
        let ex = WordExtractor { use_text_flow: true, ..Default::default() };
        assert_eq!(ex.extract_words(&chars).iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn blank_chars_terminate_a_word_and_are_dropped() {
        let chars = vec![
            ch("a", 0.0, 5.0, 10.0, 20.0),
            ch(" ", 5.0, 8.0, 10.0, 20.0),
            ch("b", 8.0, 13.0, 10.0, 20.0),
        ];
        let words = WordExtractor::default().extract_words(&chars);
        assert_eq!(words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn ligatures_expand_only_at_merge_time() {
        let chars = vec![ch("\u{FB01}", 0.0, 5.0, 10.0, 20.0), ch("n", 5.0, 10.0, 10.0, 20.0)];
        let words = WordExtractor::default().extract_words(&chars);
        assert_eq!(words[0].text, "fin");
    }

    #[test]
    fn extract_text_joins_words_with_spaces_and_lines_with_newlines() {
        let chars = vec![
            ch("H", 0.0, 5.0, 10.0, 20.0),
            ch("i", 5.0, 8.0, 10.0, 20.0),
            ch("y", 20.0, 25.0, 10.0, 20.0),
            ch("o", 40.0, 45.0, 40.0, 50.0),
        ];
        let text = extract_text(&chars, &WordExtractor::default());
        assert_eq!(text, "Hi y\no");
    }

    #[test]
    fn extract_text_of_no_chars_is_empty() {
        assert_eq!(extract_text(&[], &WordExtractor::default()), "");
    }

    #[test]
    fn python_round_is_half_to_even() {
        assert_eq!(python_round(0.25, 1), 0.2);
        assert_eq!(python_round(0.35, 1), 0.3); // 0.35 is actually 0.34999... in binary
        assert_eq!(python_round(11.259_000_000_000_015, 1), 11.3);
        assert_eq!(python_round(11.258_600_000_000_001, 1), 11.3);
    }

    #[test]
    fn dedupe_drops_a_fake_bold_double_draw() {
        // Same glyph drawn twice at (near) the same spot -- the classic fake-bold
        // pattern that renders as "MMoosstt" without dedup.
        let chars = vec![
            ch("M", 0.0, 8.0, 10.0, 20.0),
            ch("M", 0.3, 8.3, 10.2, 20.2),
            ch("o", 8.0, 14.0, 10.0, 20.0),
        ];
        let kept = dedupe_chars_size_tolerant(&chars, 1.0, 1);
        assert_eq!(kept, vec![0, 2]);
    }

    #[test]
    fn dedupe_keeps_distinct_glyphs_at_the_same_spot() {
        // Different text -> different group key -> never deduped against each other.
        let chars = vec![ch("7", 0.0, 8.0, 10.0, 20.0), ch("3", 0.0, 8.0, 10.0, 20.0)];
        let kept = dedupe_chars_size_tolerant(&chars, 1.0, 1);
        assert_eq!(kept, vec![0, 1]);
    }

    #[test]
    fn dedupe_is_size_tolerant_where_plain_pdfplumber_is_not() {
        // The exact gap normalize.py's `_dedupe_chars` exists to close: two
        // genuine duplicates whose sizes differ by a float hair.
        let mut a = ch("t", 0.0, 5.0, 10.0, 20.0);
        let mut b = ch("t", 0.2, 5.2, 10.1, 20.1);
        a.size = 11.259_000_000_000_015;
        b.size = 11.258_600_000_000_001;
        let chars = vec![a, b];
        assert_eq!(dedupe_chars_size_tolerant(&chars, 1.0, 1), vec![0]);
        // With no size rounding at all (decimals high enough to preserve the
        // difference) both survive -- which is the bug being fixed.
        assert_eq!(dedupe_chars_size_tolerant(&chars, 1.0, 12), vec![0, 1]);
    }

    #[test]
    fn dedupe_preserves_original_order() {
        let chars = vec![
            ch("c", 20.0, 25.0, 10.0, 20.0),
            ch("a", 0.0, 5.0, 10.0, 20.0),
            ch("b", 10.0, 15.0, 10.0, 20.0),
        ];
        let kept = dedupe_chars_size_tolerant(&chars, 1.0, 1);
        assert_eq!(kept, vec![0, 1, 2]);
    }
}

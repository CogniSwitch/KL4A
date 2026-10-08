//! Port of `kl4a/codekb/layout.py` -- the shared page chrome (nav, header,
//! stylesheet, small client script) that every codekb web page is rendered
//! inside of.

use serde_json::Value;

/// Mirrors `kl4a.codekb.layout.DEFAULT_VIEWS`.
pub fn default_views() -> Vec<NavView> {
    vec![NavView::Pair("code".to_string(), "Repository".to_string())]
}

/// Mirrors the `list[str] | list[tuple[str, str]]` union Python's `layout`
/// and `nav_pairs` accept for `nav_views`: either a bare route name (SOP
/// bundles, labelled by title-casing the route) or an explicit
/// `(route, label)` pair (code bundles pass pairs because a route name
/// cannot carry the label it needs).
#[derive(Debug, Clone)]
pub enum NavView {
    Route(String),
    Pair(String, String),
}

/// Mirrors `nav_pairs`: normalise a nav spec to `(route, label)` pairs.
pub fn nav_pairs(views: &[NavView]) -> Vec<(String, String)> {
    views
        .iter()
        .map(|view| match view {
            NavView::Pair(route, label) => (route.clone(), label.clone()),
            NavView::Route(route) => (route.clone(), python_title(route)),
        })
        .collect()
}

/// Mirrors `nav_item`.
pub fn nav_item(route: &str, label: &str, active: &str, base_path: &str) -> String {
    let css = if route == active { "active" } else { "" };
    format!(
        "<a class=\"{css}\" href=\"{base_path}/{route}\">{}</a>",
        escape_html(label)
    )
}

/// Mirrors `header_summary`: the one-line description beside the bundle
/// title.
///
/// Code bundles call their records claims everywhere else, so the header
/// said "items" about the same things. Pages that are not a bundle at all
/// reported "0 sources - 0 items", which is true and useless.
pub fn header_summary(summary: &Value) -> String {
    // Python: `if summary.get("text"): return str(summary["text"])` -- a
    // truthiness check (missing/None/empty-string/0 all fall through), not
    // just a presence check.
    if truthy(summary.get("text")) {
        return value_display(summary.get("text").expect("checked by truthy()"));
    }
    let has_source_count = truthy(summary.get("source_count"));
    let has_item_count = truthy(summary.get("knowledge_item_count"));
    if !has_source_count && !has_item_count {
        return summary
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    }
    let unit = summary
        .get("unit")
        .and_then(Value::as_str)
        .unwrap_or("items");
    let source_count = value_display(summary.get("source_count").unwrap_or(&Value::Null));
    let item_count = value_display(summary.get("knowledge_item_count").unwrap_or(&Value::Null));
    let status = summary
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("{source_count} sources \u{b7} {item_count} {unit} \u{b7} {status}")
}

/// Mirrors `layout`: render the page chrome.
pub fn layout(
    active: &str,
    summary: &Value,
    body: &str,
    message: &str,
    base_path: &str,
    nav_views: Option<&[NavView]>,
    searchable: bool,
) -> String {
    let owned_default;
    let views_spec: &[NavView] = match nav_views {
        Some(v) => v,
        None => {
            owned_default = default_views();
            &owned_default
        }
    };
    let views = nav_pairs(views_spec);

    // Workbench-wide config, not a bundle view: an icon pushed to the far
    // end, so it stops competing with bundle content for a slot in the strip.
    let settings_active = if active == "settings" { "active" } else { "" };
    let settings_link = format!(
        "<a class=\"nav-icon {settings_active}\" href=\"/settings\"><span class=\"glyph\" aria-hidden=\"true\">\u{2699}</span>Settings</a>"
    );

    let nav = if !base_path.is_empty() {
        let bundle_link = "<a href=\"/bundles\">Bundles</a>".to_string();
        let items: String = views
            .iter()
            .map(|(route, label)| nav_item(route, label, active, base_path))
            .collect();
        format!("{bundle_link}{items}{settings_link}")
    } else if active == "bundles" {
        format!("<a class=\"active\" href=\"/bundles\">Bundles</a>{settings_link}")
    } else if active == "settings" {
        format!("<a href=\"/bundles\">Bundles</a>{settings_link}")
    } else {
        views
            .iter()
            .map(|(route, label)| nav_item(route, label, active, base_path))
            .collect()
    };

    let message_html = if !message.is_empty() {
        format!("<div class=\"notice\">{}</div>", escape_html(message))
    } else {
        String::new()
    };

    let search_box = if !base_path.is_empty() && searchable {
        format!(
            "<form class=\"header-search\" method=\"get\" action=\"{base_path}/search\" role=\"search\"><input type=\"text\" name=\"q\" placeholder=\"Search this bundle\" aria-label=\"Search this bundle\"><button>Search</button></form>"
        )
    } else {
        String::new()
    };

    let title = summary.get("title").and_then(Value::as_str).unwrap_or("");
    let title_escaped = escape_html(title);
    let header_summary_escaped = escape_html(&header_summary(summary));

    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>");
    html.push_str(&title_escaped);
    html.push_str(" - SOP Knowledge Workbench</title>\n  <style>\n");
    html.push_str(STYLE_CSS);
    html.push_str("\n  </style>\n</head>\n<body>\n  <header><h1>");
    html.push_str(&title_escaped);
    html.push_str("</h1>");
    html.push_str(&search_box);
    html.push_str("<div class=\"summary\">");
    html.push_str(&header_summary_escaped);
    html.push_str("</div></header>\n  <nav>");
    html.push_str(&nav);
    html.push_str("</nav>\n  <main>");
    html.push_str(&message_html);
    html.push_str(body);
    html.push_str("</main>\n  <script>\n");
    html.push_str(CLIENT_SCRIPT);
    html.push_str("\n  </script>\n</body>\n</html>");
    html
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// Renders a JSON scalar the way an f-string would stringify it (numbers
/// without a trailing `.0` for integers, etc.) for the `source_count` /
/// `knowledge_item_count` interpolations.
fn value_display(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => "None".to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        other => other.to_string(),
    }
}

/// Python's `str.title()`: capitalises the first letter of every run of
/// alphabetic characters and lowercases the rest of that run.
fn python_title(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut prev_is_alpha = false;
    for c in input.chars() {
        if c.is_alphabetic() {
            if prev_is_alpha {
                result.extend(c.to_lowercase());
            } else {
                result.extend(c.to_uppercase());
            }
            prev_is_alpha = true;
        } else {
            result.push(c);
            prev_is_alpha = false;
        }
    }
    result
}

/// Mirrors Python's `html.escape` (used as the bare `escape(...)` call
/// throughout `layout.py`): escapes `&`, `<`, `>`, `"` and `'`.
pub fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// The page-chrome stylesheet, copied verbatim from `layout.py`'s `layout()`
/// f-string (its literal `{{`/`}}` un-escaped back to `{`/`}` since this is a
/// plain string, not a format template -- there is no per-render
/// interpolation inside the stylesheet itself).
const STYLE_CSS: &str = r#":root { color-scheme: light; --ink:#1f2933; --muted:#5f6b7a; --line:#d8dee8; --bg:#f7f8fa; --panel:#ffffff; --accent:#176b87; --ok:#23734d; --warn:#936316; --bad:#a13d3d; --source:#176b87; --section:#6f5aa8; --knowledge:#23734d; --review:#936316; --evidence:#4f657a; }
* { box-sizing:border-box; }
body { margin:0; font:14px/1.45 system-ui, -apple-system, Segoe UI, sans-serif; color:var(--ink); background:var(--bg); overflow-x:hidden; }
header { display:flex; gap:20px; align-items:center; justify-content:space-between; padding:14px 22px; border-bottom:1px solid var(--line); background:var(--panel); flex-wrap:wrap; }
/* One box over symbols, claims, endpoints, files and documents, so finding
   something no longer starts with guessing which tab owns it. */
.header-search { display:flex; gap:7px; flex:1; max-width:460px; min-width:220px; margin:0; }
.header-search input { flex:1; }
h1 { font-size:18px; margin:0; font-weight:700; }
h2 { font-size:16px; margin:0 0 12px; }
h3 { font-size:14px; margin:0 0 8px; }
/* The selected tab was a white box with a light grey border on a light grey
   bar -- a few percent of contrast, and less than that on a 15px glyph. The
   current section now carries the accent colour and an underline, and the
   rest read as muted-but-clickable rather than as static text. */
nav { display:flex; gap:2px; padding:0 20px; border-bottom:1px solid var(--line); background:#eef2f5; overflow-x:auto; align-items:stretch; }
nav a { color:var(--muted); text-decoration:none; padding:11px 12px; white-space:nowrap;
         border-bottom:3px solid transparent; }
nav a:hover { color:var(--ink); background:rgba(255,255,255,.6); }
nav a.active { color:var(--accent); font-weight:600; background:var(--panel); border-bottom-color:var(--accent); }
nav a.nav-icon { margin-left:auto; display:flex; align-items:center; gap:7px; }
nav a.nav-icon .glyph { font-size:15px; line-height:1; }
main { padding:18px 22px 28px; max-width:1680px; margin:0 auto; }
table { width:100%; border-collapse:collapse; background:var(--panel); border:1px solid var(--line); table-layout:fixed; }
th, td { padding:9px 10px; border-bottom:1px solid var(--line); text-align:left; vertical-align:top; overflow-wrap:anywhere; }
th { font-size:12px; color:var(--muted); text-transform:uppercase; }
pre, textarea, input, select { font:13px/1.4 ui-monospace, SFMono-Regular, Consolas, monospace; }
pre { white-space:pre-wrap; background:#fbfcfd; border:1px solid var(--line); padding:12px; overflow:auto; max-height:520px; overflow-wrap:anywhere; }
textarea, input, select { width:100%; border:1px solid var(--line); border-radius:6px; padding:7px; background:#fff; color:var(--ink); }
input[type="checkbox"] { width:16px; height:16px; padding:0; flex:0 0 auto; }
label { display:grid; gap:5px; }
label.choice { display:flex; align-items:center; gap:8px; color:var(--ink); }
button { border:1px solid var(--accent); background:var(--accent); color:#fff; border-radius:6px; padding:7px 10px; cursor:pointer; }
button.secondary { color:var(--accent); background:#fff; }
.grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(320px,1fr)); gap:14px; }
.split { display:grid; grid-template-columns:minmax(260px,360px) minmax(0,1fr); gap:14px; align-items:start; }
.agent-layout { display:grid; grid-template-columns:minmax(340px,520px) minmax(0,1fr); gap:14px; align-items:start; }
.review-layout,.concept-layout,.graph-layout { display:grid; grid-template-columns:minmax(280px,390px) minmax(0,1fr); gap:14px; align-items:start; }
.panel,.message,.split,.grid,.agent-layout,.review-layout,.concept-layout,.graph-layout { min-width:0; }
.panel { background:var(--panel); border:1px solid var(--line); border-radius:8px; padding:14px; }
.muted { color:var(--muted); }
.pill { display:inline-block; padding:2px 7px; border-radius:999px; background:#edf3f5; color:var(--accent); font-size:12px; }
/* One visual, one meaning. `.pill` used to be a status badge, a type tag, an
   action button, a navigation control and a metadata chip all at once, which
   is why a page of them read as noise. The four below are not
   interchangeable: a tag names a kind, a status carries state and colour, a
   segmented control switches view, and a button acts. */
.tag { display:inline-block; padding:2px 7px; border-radius:4px; background:#eef1f4;
        border:1px solid var(--line); color:var(--muted); font-size:12px; white-space:nowrap; }
.tag.tier4 { color:var(--section); border-color:#cfc4e8; background:#f4f1fb; font-weight:600; }
.tag.required { color:var(--accent); border-color:#b9d3dc; background:#eef5f8; font-weight:600; }
.field-head { display:flex; gap:8px; align-items:center; flex-wrap:wrap; }
/* A chip that lands directly in a grid label would otherwise stretch to the
   full column width and stop reading as a chip. */
label > .tag, label > .status { justify-self:start; }
.status { display:inline-block; padding:2px 9px; border-radius:999px; background:#f7f9fb;
           border:1px solid var(--line); color:var(--muted); font-size:12px; white-space:nowrap; }
.status.approved, .status.exact, .status.ok { color:var(--ok); border-color:#b6d5c5; background:#f2f9f5; }
.status.deferred, .status.proposed, .status.edited, .status.inferred, .status.warn {
    color:var(--warn); border-color:#e2d0ab; background:#fdf8ee; }
.status.rejected, .status.unresolved, .status.bad { color:var(--bad); border-color:#e3bfbf; background:#fdf3f3; }
.segmented { display:flex; flex-wrap:wrap; margin:0 0 16px; border:1px solid var(--line);
              border-radius:7px; overflow:hidden; background:var(--panel); width:fit-content; max-width:100%; }
.segmented a { padding:7px 14px; color:var(--ink); text-decoration:none; font-size:13px;
                border-right:1px solid var(--line); white-space:nowrap; }
.segmented a:last-child { border-right:0; }
.segmented a.active { background:var(--accent); color:#fff; font-weight:600; }
.button-link { display:inline-block; border:1px solid var(--accent); background:var(--accent); color:#fff;
                border-radius:6px; padding:6px 11px; font-size:13px; text-decoration:none; }
.button-link.secondary { color:var(--accent); background:#fff; }
/* Long qualified names shattered across lines under `table-layout:fixed`;
   code tables size to content and scroll inside their wrapper instead. */
table.code-table { table-layout:auto; }
table.code-table .qname { overflow-wrap:normal; white-space:nowrap; }
.approved { color:var(--ok); } .rejected { color:var(--bad); } .deferred,.proposed,.edited { color:var(--warn); }
.row-actions { display:grid; grid-template-columns:repeat(5,minmax(74px,1fr)); gap:6px; margin-top:8px; }
.inline-form { display:grid; gap:9px; margin-top:8px; }
.inline-form button { justify-self:start; }
.notice { margin:0 0 14px; padding:10px 12px; border:1px solid #b6d5c5; background:#eef8f2; border-radius:8px; }
.summary { color:var(--muted); font-size:13px; }
.toolbar { display:flex; gap:8px; flex-wrap:wrap; align-items:center; margin:0 0 14px; }
.source-list a { display:block; padding:8px 10px; border:1px solid var(--line); border-radius:6px; margin-bottom:7px; color:var(--ink); text-decoration:none; background:#fff; }
.source-list a.active { border-color:var(--accent); box-shadow:inset 3px 0 0 var(--accent); }
.rail { display:grid; gap:7px; max-height:calc(100vh - 190px); overflow:auto; padding-right:2px; }
.rail a { display:block; padding:9px 10px; border:1px solid var(--line); border-radius:6px; color:var(--ink); text-decoration:none; background:#fff;
           overflow:hidden; overflow-wrap:anywhere; }
.rail a.active { border-color:var(--accent); box-shadow:inset 3px 0 0 var(--accent); }
/* A long qualified name used to overflow the row and cover the count. In a
   narrow rail wrapping beats truncating: two names can share a prefix and
   differ only at the end. */
.rail-count { float:right; color:var(--muted); font-size:12px; margin-left:8px; }
.detail-header { display:flex; justify-content:space-between; gap:12px; align-items:start; margin-bottom:10px; }
.detail-header h3 { margin:0; }
.table-wrap { width:100%; overflow:auto; }
.stats { display:grid; grid-template-columns:repeat(auto-fit,minmax(140px,1fr)); gap:10px; margin:0 0 14px; }
.stat { background:var(--panel); border:1px solid var(--line); border-radius:8px; padding:12px; }
.stat strong { display:block; font-size:20px; }
.graph-wrap { width:100%; overflow:auto; border:1px solid var(--line); border-radius:8px; background:#fff; }
.graph-node text { font:12px system-ui, sans-serif; fill:#1f2933; }
.chat { display:grid; gap:12px; }
.message { border:1px solid var(--line); border-radius:8px; background:#fff; padding:12px; }
.message.user { border-left:4px solid var(--accent); }
.message.agent { border-left:4px solid var(--ok); }
.message-meta { display:flex; gap:8px; flex-wrap:wrap; align-items:center; margin-bottom:8px; color:var(--muted); font-size:12px; }
.scenario-box { min-height:220px; font-family:system-ui, -apple-system, Segoe UI, sans-serif; }
.picker-group { border:1px solid var(--line); border-radius:8px; padding:12px; background:#fbfcfd; display:grid; gap:10px; }
.picker-actions { display:grid; grid-template-columns:repeat(auto-fit,minmax(190px,1fr)); gap:10px; }
.picker-action { border:1px dashed var(--line); border-radius:8px; padding:10px; background:#fff; }
.picker-action input { margin-top:7px; }
.option-grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(210px,1fr)); gap:10px; }
.option-box { border:1px solid var(--line); border-radius:8px; padding:10px; background:#fff; min-height:56px; }
.option-box label.choice { height:100%; }
details.message summary { cursor:pointer; font-weight:700; }
details.message[open] summary { margin-bottom:8px; }
.answer-pre { max-height:620px; }
.confidence-editor { max-width:180px; }
.hidden { display:none !important; }
td pre { max-height:220px; }
@media (max-width: 760px) {
  header { align-items:flex-start; padding:12px 16px; }
  h1 { font-size:17px; max-width:62%; }
  main { padding:16px 14px 24px; }
  nav { padding:9px 14px; max-width:100vw; }
  .split,.agent-layout,.review-layout,.concept-layout,.graph-layout { grid-template-columns:1fr; }
  .row-actions { grid-template-columns:repeat(2,minmax(74px,1fr)); }
  .rail { max-height:320px; }
  table { min-width:520px; }
  .detail-header { display:block; }
  .scenario-box { min-height:180px; }
}"#;

/// The client-side script, copied verbatim from `layout.py`'s `layout()`
/// f-string (its literal `{{`/`}}` un-escaped back to `{`/`}`).
const CLIENT_SCRIPT: &str = r#"document.addEventListener('change', (event) => {
  const field = event.target;
  if (!field.matches('[data-edit-field]')) return;
  const form = field.closest('form');
  const textEditor = form.querySelector('[data-edit-text]');
  const numberEditor = form.querySelector('[data-edit-number]');
  const selected = field.options[field.selectedIndex];
  const value = selected ? selected.dataset.value || '' : '';
  if (field.value === 'confidence') {
    textEditor.classList.add('hidden');
    textEditor.disabled = true;
    numberEditor.classList.remove('hidden');
    numberEditor.disabled = false;
    numberEditor.value = value;
  } else {
    numberEditor.classList.add('hidden');
    numberEditor.disabled = true;
    textEditor.classList.remove('hidden');
    textEditor.disabled = false;
    textEditor.value = value;
  }
});"#;

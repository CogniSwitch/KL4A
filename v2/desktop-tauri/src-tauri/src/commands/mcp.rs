//! §4.10 MCP. `sopkb-mcp` (CLI shape: `sopkb-mcp <bundle_dir>
//! [--enable-review-notes]`, confirmed from `sopkb-rust/bin/sopkb-mcp/src/main.rs`)
//! is a separate binary this app never launches itself; `get_mcp_invocation` only
//! tells the caller how to launch it. `list_mcp_client_targets`/
//! `configure_mcp_client` below are a genuinely new, no-Python-equivalent
//! capability ("one-click MCP client configuration") that goes one step further:
//! for a host this app can locate and safely edit/drive, it writes the invocation
//! into that host's own config on the user's behalf, with a copy-only fallback
//! wherever it can't confirm that's safe.
//!
//! Safety rules for every write in this module (these matter -- a mistake here
//! corrupts config belonging to a DIFFERENT application, not this one):
//! - Never fabricate a client's config directory. If it doesn't already exist,
//!   that client is reported as not-located rather than silently creating a
//!   fresh config tree nothing will ever read.
//! - Never clobber an existing entry under the same name without `force`, and
//!   never touch any OTHER entry/key in an existing file.
//! - Back up the target file before writing, whenever one already existed.
//! - Prefer shelling out to a client's own `mcp add`/`mcp remove` subcommand
//!   over hand-editing its config format, whenever one exists and is documented
//!   (Claude Code, Codex) -- that CLI owns its own schema and already refuses to
//!   clobber a same-named entry on its own, which is strictly safer than this
//!   app re-implementing that file's format. Claude Desktop has no such CLI, so
//!   its `claude_desktop_config.json` is edited directly (JSON merge, see
//!   `merge_claude_desktop_config`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, CmdResult};
use crate::state::{resolve_bundle_dir, AppState};

/// `Command::new(program)`, but with Windows' console window suppressed. Every
/// spawn in this module is a background probe/config-write the user never asked
/// to see a terminal for -- without this, a GUI-subsystem app spawning `cmd.exe`
/// (or any console-subsystem child) still briefly flashes a visible console
/// window on screen (a real, reported symptom: "two command prompts quickly
/// opening & closing" every time Settings loads, one per CLI client probed by
/// `list_mcp_client_targets`). `CREATE_NO_WINDOW` (0x08000000) is the standard
/// fix; it's a no-op to set on any other platform, so this only special-cases
/// Windows rather than needing a `#[cfg]` at every call site.
fn silent_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct McpInvocation {
    pub command: String,
    pub args: Vec<String>,
    pub enable_review_notes_flag: String,
    /// The exact name this bundle would be registered under in a host's MCP
    /// config (see `entry_name_for`) -- surfaced here so the frontend can build
    /// a manual-setup snippet for a not-located client using the SAME name the
    /// backend would use for an automatic one, rather than re-deriving it from
    /// `args[0]` (the bundle dir path) and risking drift from `entry_name_for`'s
    /// own logic.
    pub entry_name: String,
}

/// Pure shaping, unit-tested independent of the binary-resolution IO below.
pub(crate) fn build_mcp_invocation(command: String, bundle_dir: &Path) -> McpInvocation {
    McpInvocation {
        command,
        args: vec![bundle_dir.display().to_string()],
        enable_review_notes_flag: "--enable-review-notes".to_string(),
        entry_name: entry_name_for(bundle_dir),
    }
}

/// Same sibling-of-executable convention the old `sidecar.rs` used for its frozen
/// binary lookup (`frozen_candidates`, now deleted): a bundled app ships `sopkb-mcp`
/// next to its own executable. Falls back to the bare name (relying on `PATH`, or on
/// `cargo build -p sopkb-mcp` having dropped it in the same `target/` dir during
/// `cargo tauri dev`) when no sibling binary is found -- still a usable, copyable
/// invocation, just not an absolute path.
fn resolve_mcp_command() -> String {
    let exe_name = format!("sopkb-mcp{}", std::env::consts::EXE_SUFFIX);
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(&exe_name);
            if candidate.is_file() {
                return candidate.display().to_string();
            }
        }
    }
    exe_name
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_mcp_invocation(state: State<AppState>, key: Option<String>) -> CmdResult<McpInvocation> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    Ok(build_mcp_invocation(resolve_mcp_command(), &bundle_dir))
}

// ---------------------------------------------------------------------------
// One-click client configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientId {
    ClaudeDesktop,
    ClaudeCode,
    Codex,
}

impl ClientId {
    const ALL: [ClientId; 3] = [ClientId::ClaudeDesktop, ClientId::ClaudeCode, ClientId::Codex];

    fn as_str(self) -> &'static str {
        match self {
            ClientId::ClaudeDesktop => "claude-desktop",
            ClientId::ClaudeCode => "claude-code",
            ClientId::Codex => "codex",
        }
    }

    fn label(self) -> &'static str {
        match self {
            ClientId::ClaudeDesktop => "Claude Desktop",
            ClientId::ClaudeCode => "Claude Code",
            ClientId::Codex => "Codex (CLI & Desktop)",
        }
    }

    /// `None` for Claude Desktop -- it has no CLI, it's a direct file edit.
    fn cli_name(self) -> Option<&'static str> {
        match self {
            ClientId::ClaudeDesktop => None,
            ClientId::ClaudeCode => Some("claude"),
            ClientId::Codex => Some("codex"),
        }
    }

    fn parse(s: &str) -> CmdResult<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s).ok_or_else(|| AppError::invalid_input(format!("unknown MCP client id: {s}")))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct McpClientTarget {
    pub id: String,
    pub label: String,
    /// `"config-file"` (this app edits the client's config JSON directly) or
    /// `"cli"` (this app shells out to that client's own `mcp add`).
    pub method: String,
    pub located: bool,
    /// The config file path (`config-file`) or resolved binary path (`cli`),
    /// when `located` is true.
    pub location: Option<String>,
    /// Set when `located` is false, explaining why and what to do instead.
    pub note: Option<String>,
    /// `config-file` clients only: where that config file WOULD go even when
    /// `located` is false (e.g. `%APPDATA%\Claude\claude_desktop_config.json`
    /// still resolves even if the `Claude` folder doesn't exist yet) -- lets the
    /// frontend give concrete manual-setup instructions instead of a dead end.
    /// Always `None` for `cli` clients, which have no single fixed path to name.
    pub default_location_hint: Option<String>,
}

/// App-managed cache of the last MCP-client detection run (see `detect_mcp_client_targets`).
/// Detection means spawning a probe process per CLI-based client (`cmd /C where <name>` /
/// `which <name>`) -- real, visible cost on every call (a console-window flash on Windows
/// pre-`silent_command`, and just wasted work everywhere) that has no reason to repeat on
/// every Settings mount. Populated once by a background task at startup
/// (`spawn_mcp_detection_startup_task`) and only re-populated on an explicit user request
/// (`rescan_mcp_client_targets`) -- never implicitly on read.
#[derive(Default)]
pub struct McpDetectionCache(Mutex<Option<Vec<McpClientTarget>>>);

impl McpDetectionCache {
    fn get(&self) -> Option<Vec<McpClientTarget>> {
        self.0.lock().expect("mcp detection cache poisoned").clone()
    }

    fn set(&self, targets: Vec<McpClientTarget>) {
        *self.0.lock().expect("mcp detection cache poisoned") = Some(targets);
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct McpConfigureResult {
    /// `"configured"` | `"already_configured"` | `"not_found"`.
    pub outcome: String,
    pub message: String,
    /// Set only on the Claude Desktop config-file path, only when a file
    /// already existed there before this write.
    pub backup_path: Option<String>,
}

/// A distinctive, bundle-specific name, never a generic `"sopkb"` -- two bundles
/// configured for the same host must not collide, and a generic name would
/// silently clobber (or refuse to add, once `force` is required) a second
/// bundle's entry.
fn entry_name_for(bundle_dir: &Path) -> String {
    let key = bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("bundle");
    format!("sopkb-{key}")
}

/// Confirmed against `%APPDATA%\Claude\claude_desktop_config.json` on Windows /
/// `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS,
/// per Anthropic's own MCP quickstart (2026-08-25). `tauri`'s `config_dir()`
/// resolves to exactly `%APPDATA%`/`~/Library/Application Support` on those two
/// platforms respectively, so no extra platform-specific-path code is needed
/// here. Known gap, disclosed rather than silently mishandled: a Microsoft-
/// Store/MSIX install of Claude Desktop reads from a virtualized path under
/// `%LOCALAPPDATA%\Packages\...` instead -- undetectable from here, so an MSIX
/// install will read back as "not located" (the plain `%APPDATA%\Claude` folder
/// won't exist) rather than this app writing somewhere that install never reads.
fn claude_desktop_config_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().config_dir().ok().map(|dir| dir.join("Claude"))
}

fn claude_desktop_config_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = claude_desktop_config_dir(app)?;
    if dir.is_dir() {
        Some(dir.join("claude_desktop_config.json"))
    } else {
        None
    }
}

/// Where `claude_desktop_config.json` would live even if `Claude`'s own folder
/// doesn't exist yet -- unlike [`claude_desktop_config_path`], never gated on
/// the directory actually existing. Purely instructional (see
/// `McpClientTarget::default_location_hint`'s own doc comment).
fn claude_desktop_config_path_hint(app: &AppHandle) -> Option<PathBuf> {
    claude_desktop_config_dir(app).map(|dir| dir.join("claude_desktop_config.json"))
}

/// Locates the real, invocable path behind a bare CLI name (`claude`, `codex`).
/// Works around a real Windows gap: `std::process::Command`'s CreateProcess-based
/// search does not consult `PATHEXT` the way an interactive shell does, so a bare
/// `Command::new("claude")` can silently fail to find an npm-installed
/// `claude.cmd` shim even though `claude` works fine typed at a prompt. `cmd /C
/// where <name>` performs the same PATHEXT-aware search a real shell does and
/// prints every match, one per line, in PATHEXT priority order. A `.ps1`-only
/// match is skipped -- `CreateProcess` has no association for it and this app
/// has no reason to add a PowerShell-hosting layer just to invoke one; a CLI
/// that only ships a `.ps1` entry point is reported as not found here (falls
/// back to copy-only) rather than silently mis-invoked.
fn resolve_cli_binary(name: &str) -> Option<PathBuf> {
    let output = if cfg!(windows) {
        silent_command("cmd").args(["/C", "where", name]).output().ok()?
    } else {
        silent_command("which").arg(name).output().ok()?
    };
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.to_ascii_lowercase().ends_with(".ps1"))
        .map(PathBuf::from)
}

/// The exact argv this app hands to `claude`/`codex`'s own `mcp add` subcommand
/// -- kept pure and unit-tested independent of either CLI being installed.
/// `--` always precedes the server's own command so a flag-shaped argument of
/// the *server's* own (`--enable-review-notes`) is never misparsed as a flag of
/// the host CLI's `mcp add`. `--scope user` is Claude-Code-specific (registers
/// once for all projects, matching "a bundle's MCP entry isn't tied to
/// whichever directory this app happened to be launched from"); Codex's `mcp
/// add` has no scope concept in its own docs, so none is passed. Confirmed live
/// against a real `claude` 2.x install on 2026-08-25 (add, the "already exists"
/// collision, and remove all round-tripped exactly as designed); Codex's syntax
/// is analogous per OpenAI's own docs but was not available to test live on
/// this machine -- if it drifts, `configure_via_cli`'s error path still surfaces
/// the real stderr rather than a misleading generic message.
fn build_add_args(client: ClientId, entry_name: &str, command: &str, args: &[String]) -> Vec<String> {
    let mut argv = vec!["mcp".to_string(), "add".to_string()];
    if client == ClientId::ClaudeCode {
        argv.push("--scope".to_string());
        argv.push("user".to_string());
    }
    argv.push(entry_name.to_string());
    argv.push("--".to_string());
    argv.push(command.to_string());
    argv.extend_from_slice(args);
    argv
}

fn build_remove_args(client: ClientId, entry_name: &str) -> Vec<String> {
    let mut argv = vec!["mcp".to_string(), "remove".to_string(), entry_name.to_string()];
    if client == ClientId::ClaudeCode {
        argv.push("--scope".to_string());
        argv.push("user".to_string());
    }
    argv
}

/// Merges one `mcpServers.<entry_name>` entry into an existing (or absent)
/// `claude_desktop_config.json` document. Touches nothing else in the file --
/// any other server entries or top-level keys survive untouched. `Err` (mapped
/// by the caller to `"already_configured"`) when `entry_name` already exists
/// and `force` is false.
fn merge_claude_desktop_config(existing: &Value, entry_name: &str, command: &str, args: &[String], force: bool) -> Result<Value, String> {
    let mut root = if existing.is_object() { existing.clone() } else { serde_json::json!({}) };
    let obj = root.as_object_mut().expect("just ensured this is an object");

    let servers = obj.entry("mcpServers").or_insert_with(|| serde_json::json!({}));
    if !servers.is_object() {
        *servers = serde_json::json!({});
    }
    let servers_obj = servers.as_object_mut().expect("just ensured this is an object");

    if servers_obj.contains_key(entry_name) && !force {
        return Err(format!(
            "an MCP server named \"{entry_name}\" is already configured in this file -- remove it there first, or confirm again to overwrite it"
        ));
    }
    servers_obj.insert(entry_name.to_string(), serde_json::json!({ "command": command, "args": args }));
    Ok(root)
}

/// Blocking file I/O: read (if present) -> merge -> back up the original (if one
/// existed) -> write. Free function, no `AppHandle`/`State`, so it's directly
/// unit-testable against a tempdir path standing in for the real config file.
fn configure_claude_desktop(path: &Path, entry_name: &str, command: &str, args: &[String], force: bool) -> CmdResult<McpConfigureResult> {
    let existing_text = fs::read_to_string(path).ok();
    let existing: Value = existing_text.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_else(|| serde_json::json!({}));

    let merged = match merge_claude_desktop_config(&existing, entry_name, command, args, force) {
        Ok(v) => v,
        Err(message) => return Ok(McpConfigureResult { outcome: "already_configured".to_string(), message, backup_path: None }),
    };

    let backup_path = match existing_text {
        Some(_) => {
            let backup = path.with_extension("json.bak");
            fs::copy(path, &backup).map_err(|err| AppError::new("Io", format!("could not back up {}: {err}", path.display())))?;
            Some(backup.display().to_string())
        }
        None => None,
    };

    let pretty = serde_json::to_string_pretty(&merged).map_err(|err| AppError::new("Format", format!("could not render config: {err}")))?;
    fs::write(path, pretty).map_err(|err| AppError::new("Io", format!("could not write {}: {err}", path.display())))?;

    Ok(McpConfigureResult { outcome: "configured".to_string(), message: format!("Added \"{entry_name}\" to {}", path.display()), backup_path })
}

/// Blocking process spawn against an ALREADY-RESOLVED binary path -- callers
/// (the startup/rescan detection pass, or `configure_mcp_client` reading the
/// cache) own resolution; this function never re-probes PATH itself, so
/// clicking "Configure automatically" never repeats the `where`/`which` spawn
/// detection already paid for. Optionally remove-then-add first (when
/// `force`, to get past that CLI's own "already exists" refusal) -- a
/// remove-before-force failure is deliberately swallowed, since an entry that
/// doesn't exist yet is not itself an error, and any other failure just falls
/// through to the add below, which reports its own real error if the entry
/// somehow still exists.
fn configure_via_cli(client: ClientId, cli_name: &str, binary: &Path, entry_name: &str, command: &str, args: &[String], force: bool) -> CmdResult<McpConfigureResult> {
    if force {
        let _ = silent_command(binary).args(build_remove_args(client, entry_name)).output();
    }

    let output = silent_command(binary)
        .args(build_add_args(client, entry_name, command, args))
        .output()
        .map_err(|err| AppError::new("Io", format!("could not run {}: {err}", binary.display())))?;

    if output.status.success() {
        let message = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(McpConfigureResult {
            outcome: "configured".to_string(),
            message: if message.is_empty() { format!("Added \"{entry_name}\" via {cli_name}") } else { message },
            backup_path: None,
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.to_ascii_lowercase().contains("already exists") {
        return Ok(McpConfigureResult { outcome: "already_configured".to_string(), message: stderr, backup_path: None });
    }
    Err(AppError::new("Io", if stderr.is_empty() { format!("{cli_name} mcp add failed") } else { stderr }))
}

/// The result `configure_mcp_client` returns immediately -- no process spawn,
/// no file read -- when the cache says a client isn't located. Kept as a pure
/// function so both the real command and its tests can assert on the exact
/// wording without a full Tauri `AppHandle`.
fn not_found_result(client: ClientId) -> McpConfigureResult {
    let message = match client.cli_name() {
        Some(cli_name) => {
            format!("Could not find the `{cli_name}` CLI on PATH -- install it, or use the copy button above and configure it by hand.")
        }
        None => "Could not locate Claude Desktop's config folder automatically -- it may not be installed. Use the copy button \
                  above and paste it in via Settings > Developer > Edit Config."
            .to_string(),
    };
    McpConfigureResult { outcome: "not_found".to_string(), message, backup_path: None }
}

/// The actual detection pass: one blocking probe per known client (a config-
/// dir existence check for Claude Desktop, a `where`/`which` spawn for each
/// CLI client). Deliberately synchronous and free of any `#[tauri::command]`/
/// `State` machinery so it's reusable from both the one-time startup task and
/// the explicit `rescan_mcp_client_targets` command, and callable directly
/// from a test.
fn detect_mcp_client_targets(app: &AppHandle) -> Vec<McpClientTarget> {
    ClientId::ALL
        .into_iter()
        .map(|client| match client.cli_name() {
            None => {
                let path = claude_desktop_config_path(app);
                let note = path.is_none().then(|| {
                    "Claude Desktop's config folder was not found -- it may not be installed, or was installed via the \
                     Microsoft Store (which uses a different, virtualized config location this app cannot reach \
                     automatically)."
                        .to_string()
                });
                McpClientTarget {
                    id: client.as_str().to_string(),
                    label: client.label().to_string(),
                    method: "config-file".to_string(),
                    located: path.is_some(),
                    location: path.map(|p| p.display().to_string()),
                    note,
                    default_location_hint: claude_desktop_config_path_hint(app).map(|p| p.display().to_string()),
                }
            }
            Some(cli_name) => {
                let binary = resolve_cli_binary(cli_name);
                let note = binary.is_none().then(|| format!("Could not find the `{cli_name}` CLI on PATH."));
                McpClientTarget {
                    id: client.as_str().to_string(),
                    label: client.label().to_string(),
                    method: "cli".to_string(),
                    located: binary.is_some(),
                    location: binary.map(|p| p.display().to_string()),
                    note,
                    default_location_hint: None,
                }
            }
        })
        .collect()
}

/// Runs `detect_mcp_client_targets` once in the background and populates the
/// cache -- called exactly once from `lib.rs`'s `.setup()` hook, never from a
/// command. A failure here (the `spawn_blocking` task itself panicking) is
/// logged via the app-level `startup_log` and otherwise swallowed: the cache
/// simply stays empty (`list_mcp_client_targets` keeps reporting "still
/// detecting"), which the user can recover from with the Rescan button --
/// this must never be allowed to block or fail app startup itself.
pub fn spawn_mcp_detection_startup_task(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let detect_handle = app.clone();
        match tauri::async_runtime::spawn_blocking(move || detect_mcp_client_targets(&detect_handle)).await {
            Ok(targets) => {
                crate::startup_log::log(&format!("MCP client detection complete ({} targets known)", targets.len()));
                app.state::<McpDetectionCache>().set(targets);
            }
            Err(err) => {
                crate::startup_log::log(&format!("MCP client detection task failed to complete: {err}"));
            }
        }
    });
}

/// Fast, synchronous read of the cache populated at startup (or by the last
/// `rescan_mcp_client_targets` call) -- never itself spawns a detection probe.
/// `None` means detection genuinely hasn't finished yet (a narrow startup
/// race); the frontend shows a brief "detecting..." state rather than an
/// empty list in that case.
#[tauri::command(rename_all = "snake_case")]
pub fn list_mcp_client_targets(cache: State<McpDetectionCache>) -> CmdResult<Option<Vec<McpClientTarget>>> {
    Ok(cache.get())
}

/// The ONLY place detection is allowed to re-run after startup -- an explicit
/// user click on "Rescan", never implicit. Re-probes everything fresh and
/// replaces the cache with the new result.
#[tauri::command(rename_all = "snake_case")]
pub async fn rescan_mcp_client_targets(app: AppHandle, cache: State<'_, McpDetectionCache>) -> CmdResult<Vec<McpClientTarget>> {
    let detect_handle = app.clone();
    let targets = tauri::async_runtime::spawn_blocking(move || detect_mcp_client_targets(&detect_handle))
        .await
        .map_err(|err| AppError::new("Io", format!("client detection task did not complete: {err}")))?;
    cache.set(targets.clone());
    Ok(targets)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn configure_mcp_client(state: State<'_, AppState>, cache: State<'_, McpDetectionCache>, client_id: String, key: Option<String>, force: bool) -> CmdResult<McpConfigureResult> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let client = ClientId::parse(&client_id)?;
    let entry_name = entry_name_for(&bundle_dir);
    let invocation = build_mcp_invocation(resolve_mcp_command(), &bundle_dir);

    // Read the cache the startup task (or the last Rescan) already populated --
    // never re-probe PATH/the config dir here. A client the cache doesn't know
    // about yet, or knows to be not-located, short-circuits to `not_found`
    // with zero process spawns / file reads.
    let cached_target = cache.get().and_then(|targets| targets.into_iter().find(|t| t.id == client.as_str()));
    let Some(target) = cached_target.filter(|t| t.located) else {
        return Ok(not_found_result(client));
    };
    let location = target.location.clone().expect("located implies a location was recorded");

    match client.cli_name() {
        None => {
            let path = PathBuf::from(location);
            tauri::async_runtime::spawn_blocking(move || configure_claude_desktop(&path, &entry_name, &invocation.command, &invocation.args, force))
                .await
                .map_err(|err| AppError::new("Io", format!("MCP configure task did not complete: {err}")))?
        }
        Some(cli_name) => {
            let binary = PathBuf::from(location);
            tauri::async_runtime::spawn_blocking(move || configure_via_cli(client, cli_name, &binary, &entry_name, &invocation.command, &invocation.args, force))
                .await
                .map_err(|err| AppError::new("Io", format!("MCP configure task did not complete: {err}")))?
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn build_mcp_invocation_shapes_bundle_dir_as_single_positional_arg() {
        let invocation = build_mcp_invocation("sopkb-mcp".to_string(), Path::new("/tmp/my-bundle"));
        assert_eq!(invocation.command, "sopkb-mcp");
        assert_eq!(invocation.args, vec![Path::new("/tmp/my-bundle").display().to_string()]);
        assert_eq!(invocation.enable_review_notes_flag, "--enable-review-notes");
        assert_eq!(invocation.entry_name, "sopkb-my-bundle");
    }

    #[test]
    fn entry_name_for_is_bundle_specific_not_generic() {
        assert_eq!(entry_name_for(Path::new("/bundles/hdfc-policy")), "sopkb-hdfc-policy");
        assert_ne!(entry_name_for(Path::new("/bundles/a")), entry_name_for(Path::new("/bundles/b")));
    }

    #[test]
    fn build_add_args_puts_scope_user_only_for_claude_code() {
        let claude = build_add_args(ClientId::ClaudeCode, "sopkb-x", "sopkb-mcp", &["C:/b".to_string(), "--enable-review-notes".to_string()]);
        assert_eq!(claude, vec!["mcp", "add", "--scope", "user", "sopkb-x", "--", "sopkb-mcp", "C:/b", "--enable-review-notes"]);

        let codex = build_add_args(ClientId::Codex, "sopkb-x", "sopkb-mcp", &["C:/b".to_string()]);
        assert_eq!(codex, vec!["mcp", "add", "sopkb-x", "--", "sopkb-mcp", "C:/b"]);
    }

    #[test]
    fn build_remove_args_matches_add_scope_convention() {
        assert_eq!(build_remove_args(ClientId::ClaudeCode, "sopkb-x"), vec!["mcp", "remove", "sopkb-x", "--scope", "user"]);
        assert_eq!(build_remove_args(ClientId::Codex, "sopkb-x"), vec!["mcp", "remove", "sopkb-x"]);
    }

    #[test]
    fn merge_claude_desktop_config_on_a_missing_file_creates_only_mcp_servers() {
        let merged = merge_claude_desktop_config(&Value::Null, "sopkb-x", "sopkb-mcp", &["b".to_string()], false).unwrap();
        assert_eq!(merged, serde_json::json!({"mcpServers": {"sopkb-x": {"command": "sopkb-mcp", "args": ["b"]}}}));
    }

    #[test]
    fn merge_claude_desktop_config_preserves_unrelated_keys_and_other_servers() {
        let existing = serde_json::json!({
            "someOtherTopLevelKey": true,
            "mcpServers": {"unrelated-server": {"command": "foo", "args": []}}
        });
        let merged = merge_claude_desktop_config(&existing, "sopkb-x", "sopkb-mcp", &["b".to_string()], false).unwrap();
        assert_eq!(merged["someOtherTopLevelKey"], serde_json::json!(true));
        assert_eq!(merged["mcpServers"]["unrelated-server"], serde_json::json!({"command": "foo", "args": []}));
        assert_eq!(merged["mcpServers"]["sopkb-x"], serde_json::json!({"command": "sopkb-mcp", "args": ["b"]}));
    }

    #[test]
    fn merge_claude_desktop_config_refuses_to_clobber_without_force() {
        let existing = serde_json::json!({"mcpServers": {"sopkb-x": {"command": "old", "args": []}}});
        let err = merge_claude_desktop_config(&existing, "sopkb-x", "new", &[], false).unwrap_err();
        assert!(err.contains("already configured"), "unexpected message: {err}");
    }

    #[test]
    fn merge_claude_desktop_config_overwrites_when_forced() {
        let existing = serde_json::json!({"mcpServers": {"sopkb-x": {"command": "old", "args": []}}});
        let merged = merge_claude_desktop_config(&existing, "sopkb-x", "new", &[], true).unwrap();
        assert_eq!(merged["mcpServers"]["sopkb-x"]["command"], serde_json::json!("new"));
    }

    #[test]
    fn configure_claude_desktop_backs_up_an_existing_file_before_overwriting_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("claude_desktop_config.json");
        let original = serde_json::json!({"mcpServers": {"unrelated": {"command": "keep-me", "args": []}}});
        fs::write(&path, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        let result = configure_claude_desktop(&path, "sopkb-new", "sopkb-mcp", &["b".to_string()], false).unwrap();
        assert_eq!(result.outcome, "configured");
        let backup_path = result.backup_path.expect("a backup must be made for a pre-existing file");
        assert!(Path::new(&backup_path).is_file());
        let backup: Value = serde_json::from_str(&fs::read_to_string(&backup_path).unwrap()).unwrap();
        assert_eq!(backup, original, "the backup must be the ORIGINAL content, not the merged result");

        let written: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["mcpServers"]["unrelated"], serde_json::json!({"command": "keep-me", "args": []}));
        assert_eq!(written["mcpServers"]["sopkb-new"], serde_json::json!({"command": "sopkb-mcp", "args": ["b"]}));
    }

    #[test]
    fn configure_claude_desktop_on_a_brand_new_file_makes_no_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("claude_desktop_config.json");
        let result = configure_claude_desktop(&path, "sopkb-new", "sopkb-mcp", &["b".to_string()], false).unwrap();
        assert_eq!(result.outcome, "configured");
        assert!(result.backup_path.is_none());
        assert!(!path.with_extension("json.bak").exists());
    }

    #[test]
    fn configure_claude_desktop_reports_already_configured_without_writing_when_not_forced() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("claude_desktop_config.json");
        let original = serde_json::json!({"mcpServers": {"sopkb-x": {"command": "old", "args": []}}});
        fs::write(&path, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        let result = configure_claude_desktop(&path, "sopkb-x", "new", &[], false).unwrap();
        assert_eq!(result.outcome, "already_configured");
        assert!(result.backup_path.is_none(), "a rejected write must not touch the file at all");
        let unchanged: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(unchanged, original);
    }

    #[test]
    fn configure_via_cli_no_longer_resolves_the_binary_itself_and_errors_on_a_bogus_one() {
        // Resolution is the CALLER's job now (the cache, populated by
        // `detect_mcp_client_targets`) -- `configure_via_cli` just spawns
        // whatever binary path it's handed. A path that doesn't exist is a
        // hard IO error, not a soft "not_found" outcome (that branch moved to
        // `not_found_result`, decided before this function is ever called).
        let bogus = Path::new("sopkb-definitely-not-a-real-binary-xyz");
        let err = configure_via_cli(ClientId::Codex, "codex", bogus, "sopkb-x", "sopkb-mcp", &[], false).unwrap_err();
        assert_eq!(err.kind, "Io");
    }

    #[test]
    fn not_found_result_distinguishes_config_file_clients_from_cli_clients() {
        let cli = not_found_result(ClientId::Codex);
        assert_eq!(cli.outcome, "not_found");
        assert!(cli.message.contains("`codex` CLI"), "unexpected message: {}", cli.message);

        let config_file = not_found_result(ClientId::ClaudeDesktop);
        assert_eq!(config_file.outcome, "not_found");
        assert!(config_file.message.contains("Claude Desktop"), "unexpected message: {}", config_file.message);
    }

    #[test]
    fn mcp_detection_cache_starts_empty_and_round_trips_a_set_value() {
        let cache = McpDetectionCache::default();
        assert_eq!(cache.get(), None);

        let target = McpClientTarget {
            id: "codex".to_string(),
            label: "Codex (CLI & Desktop)".to_string(),
            method: "cli".to_string(),
            located: true,
            location: Some("C:/tools/codex.exe".to_string()),
            note: None,
            default_location_hint: None,
        };
        cache.set(vec![target.clone()]);
        assert_eq!(cache.get(), Some(vec![target]));
    }

    #[test]
    fn client_id_round_trips_through_its_string_id() {
        for client in ClientId::ALL {
            assert_eq!(ClientId::parse(client.as_str()).unwrap(), client);
        }
        assert!(ClientId::parse("not-a-real-client").is_err());
    }
}

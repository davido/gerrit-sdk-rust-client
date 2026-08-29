//! Demo: read a change with detail (GET /changes/{id} -> ChangeInfo, anonymous),
//! requesting labels/votes, current revision + commit (author/committer/parent),
//! reviewers, files, and submit requirements via the `o` array option -- the fields
//! the frontend shows in its "Change Info" / "Files" panels. One REST operation.
use gerrit_client::apis::changes_api;
use gerrit_client::apis::configuration::Configuration;
use gerrit_client::models::{AccountInfo, ChangeInfo, CommitInfo, CommonFileInfo, GitPerson};
use std::collections::HashMap;
use std::sync::OnceLock;

/// ListChangesOption values that populate every panel below from a single GET.
const OPTIONS: [&str; 7] = [
    "LABELS", "DETAILED_ACCOUNTS", "DETAILED_LABELS", "CURRENT_REVISION",
    "CURRENT_COMMIT", "CURRENT_FILES", "SUBMIT_REQUIREMENTS",
];

fn main() {
    let mut url = "https://gerrit-review.googlesource.com".to_string();
    let mut change = "621763".to_string();

    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--url" => { i += 1; if let Some(v) = argv.get(i) { url = v.clone(); } }
            "--change" => { i += 1; if let Some(v) = argv.get(i) { change = v.clone(); } }
            "--no-color" => { COLOR.set(false).ok(); }
            "-h" | "--help" => {
                eprintln!("usage: get-change-detail [--url <base>] [--change <id>] [--no-color]");
                return;
            }
            other => { eprintln!("unknown arg: {other}"); std::process::exit(2); }
        }
        i += 1;
    }

    let base = url.trim_end_matches('/').to_string();
    let mut cfg = Configuration::new();
    cfg.base_path = base.clone();
    cfg.user_agent = Some("gerrit-sdk-rust-client".to_string());

    let options = Some(OPTIONS.iter().map(|s| s.to_string()).collect());
    match changes_api::get_changes_change_id(&cfg, &change, None, None, options) {
        Ok(ci) => print_change_details(&base, &ci),
        Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
    }
}

// ---- presentation -------------------------------------------------------------

fn print_change_details(base: &str, ci: &ChangeInfo) {
    print_header(base, ci);
    print_change_info(ci);
    print_people(ci);
    print_submit_requirements(ci);
    print_votes(ci);
    print_files(ci);
    println!("{}", rule());
}

fn print_header(base: &str, ci: &ChangeInfo) {
    let number = ci._number.unwrap_or_default();
    println!("{}", rule());
    println!("  {}  {}", status_badge(ci), sgr(&format!("#{number}"), BOLD));
    println!("  {}", sgr(ci.subject.as_deref().unwrap_or_default(), BOLD));
    println!("{}", rule());
    let project = ci.project.as_deref().unwrap_or_default();
    println!("  {}", fg(&format!("{base}/c/{project}/+/{number}"), BLUE_700));
}

fn print_change_info(ci: &ChangeInfo) {
    section("Change Info");
    row("Owner", &account(ci.owner.as_deref()));
    if let Some(c) = current_commit(ci) {
        row("Author", &person(c.author.as_deref()));
        row("Committer", &person(c.committer.as_deref()));
    }
    row("Repo | Branch", &format!("{} | {}",
        link(ci.project.as_deref().unwrap_or_default()),
        link(ci.branch.as_deref().unwrap_or_default())));
    row("Change-Id", &link(ci.change_id.as_deref().unwrap_or_default()));
    if let Some(t) = ci.topic.as_deref().filter(|t| !t.is_empty()) { row("Topic", &link(t)); }
    if let Some(h) = ci.hashtags.as_ref().filter(|h| !h.is_empty()) { row("Hashtags", &link(&h.join(", "))); }
    let flags = flag_chips(ci);
    if !flags.is_empty() { row("Flags", &flags.join("  ")); }
    row("Strategy", &enum_name(&format!("{:?}", ci.submit_type)));
    if let Some(parent) = parent_commit(ci) { row("Parent", &link(&short(&parent))); }
    row("Patch set", &patch_set(ci));
    row("Updated", ci.updated.as_deref().unwrap_or_default());
    row("Size", &plusminus(ci.insertions.unwrap_or_default(), ci.deletions.unwrap_or_default()));
    row("Comments", &comments_summary(ci));
}

// Reviewers / CC -- one account per line (no comma overflow).
fn print_people(ci: &ChangeInfo) {
    for (key, title) in [("REVIEWER", "Reviewers"), ("CC", "CC")] {
        let people = accounts_in(ci, key);
        if people.is_empty() { continue; }
        section(title);
        for a in people { println!("    {}", account(Some(a))); }
    }
}

fn print_submit_requirements(ci: &ChangeInfo) {
    let Some(reqs) = ci.submit_requirements.as_ref().filter(|r| !r.is_empty()) else { return };
    section("Submit Requirements");
    for r in reqs {
        let (icon, text) = req_parts(&enum_name(&format!("{:?}", r.status)));
        println!("    {icon} {:<26} {text}", r.name.as_deref().unwrap_or_default());
    }
}

fn print_votes(ci: &ChangeInfo) {
    let Some(labels) = ci.labels.as_ref().filter(|l| !l.is_empty()) else { return };
    section("Votes");
    let mut names: Vec<&String> = labels.keys().collect();
    names.sort();
    for name in names {
        // Only non-zero votes, like the UI aggregate.
        let chips: Vec<String> = labels[name].all.iter().flatten()
            .filter_map(|a| match a.value {
                Some(v) if v != 0 => Some(vote_chip(v, a.name.as_deref().unwrap_or_default())),
                _ => None,
            })
            .collect();
        let value = if chips.is_empty() { sgr("—", DIM) } else { chips.join("  ") };
        println!("    {name:<22} {value}");
    }
}

fn print_files(ci: &ChangeInfo) {
    let Some(files) = current_files(ci) else { return };
    section(&format!("Files (patch set {})", patch_set(ci)));
    let mut paths: Vec<&String> = files.keys().collect();
    // Commit message pseudo-file first, then the rest alphabetically -- like the UI.
    paths.sort_by_key(|p| (p.as_str() != "/COMMIT_MSG", p.to_lowercase()));
    for p in paths {
        let f = &files[p];
        let (letter, color) = file_status(f.status.as_deref());
        let name = if p == "/COMMIT_MSG" { "Commit message".to_string() }
            else if let Some(old) = &f.old_path { format!("{old} → {p}") }
            else { p.clone() };
        let counts = plusminus(f.lines_inserted.unwrap_or_default(), f.lines_deleted.unwrap_or_default());
        println!("    {} {name:<52} {counts}", fg(letter, color));
    }
}

fn rule() -> String {
    fg(&"─".repeat(76), HEADER_INDIGO)
}

fn section(title: &str) {
    println!();
    println!("  {}", sgr(&title.to_uppercase(), BOLD));
}

fn row(label: &str, value: &str) {
    // Pad the visible label to width BEFORE coloring -- ANSI escape bytes would
    // otherwise be counted by {:<w} and break column alignment when color is on.
    println!("    {}{value}", sgr(&format!("{label:<14}"), DIM));
}

// ---- model accessors ----------------------------------------------------------

fn current_commit(ci: &ChangeInfo) -> Option<&CommitInfo> {
    let cr = ci.current_revision.as_ref()?;
    ci.revisions.as_ref()?.get(cr)?.commit.as_deref()
}

fn current_files(ci: &ChangeInfo) -> Option<&HashMap<String, CommonFileInfo>> {
    let cr = ci.current_revision.as_ref()?;
    ci.revisions.as_ref()?.get(cr)?.files.as_ref()
}

fn parent_commit(ci: &ChangeInfo) -> Option<String> {
    current_commit(ci)?.parents.as_ref()?.first().and_then(|p| p.commit.clone())
}

/// The accounts under a reviewer-state key ("REVIEWER" / "CC"), borrowed (no clone).
fn accounts_in<'a>(ci: &'a ChangeInfo, key: &str) -> &'a [AccountInfo] {
    ci.reviewers.as_ref().and_then(|m| m.get(key)).map_or(&[], Vec::as_slice)
}

fn patch_set(ci: &ChangeInfo) -> String {
    ci.current_revision_number.map_or_else(|| "?".into(), |n| n.to_string())
}

fn short(sha: &str) -> String {
    sha.chars().take(12).collect()
}

/// Strip the `Some(...)` Debug wrapper off an Option<enum> to get the variant name.
fn enum_name(dbg: &str) -> String {
    dbg.trim_start_matches("Some(").trim_end_matches(')').to_string()
}

// ---- value formatting ---------------------------------------------------------

fn account(a: Option<&AccountInfo>) -> String {
    match a {
        Some(a) => match (a.name.as_deref(), a.email.as_deref()) {
            (Some(n), Some(e)) => named(n, e),
            (Some(n), None) => sgr(n, BOLD),
            _ => format!("account #{}", a._account_id.unwrap_or_default()),
        },
        None => "—".to_string(),
    }
}

fn person(p: Option<&GitPerson>) -> String {
    match p {
        Some(p) => named(p.name.as_deref().unwrap_or_default(), p.email.as_deref().unwrap_or_default()),
        None => "—".to_string(),
    }
}

/// An account/person: bold name, dim <email>. No blue -- reserve blue for links.
fn named(name: &str, email: &str) -> String {
    format!("{} {}", sgr(name, BOLD), sgr(&format!("<{email}>"), DIM))
}

fn flag_chips(ci: &ChangeInfo) -> Vec<String> {
    let mut f = Vec::new();
    if ci.work_in_progress.unwrap_or(false) { f.push(chip(" WIP ", WHITE, WIP_BROWN)); }
    if ci.is_private.unwrap_or(false) { f.push(chip(" Private ", WHITE, PURPLE_500)); }
    if ci.mergeable.unwrap_or(false) { f.push(fg("mergeable", GREEN_700)); }
    if ci.submittable.unwrap_or(false) { f.push(fg("submittable", GREEN_700)); }
    f
}

fn comments_summary(ci: &ChangeInfo) -> String {
    let total = ci.total_comment_count.unwrap_or_default();
    let unresolved = ci.unresolved_comment_count.unwrap_or_default();
    let resolved = total.saturating_sub(unresolved);
    // Match the UI's read: resolved in green, open (unresolved) in red when > 0.
    let open_color = if unresolved > 0 { RED_600 } else { GREEN_700 };
    format!("{total} total  ({}, {})",
        fg(&format!("{resolved} resolved"), GREEN_700),
        fg(&format!("{unresolved} unresolved"), open_color))
}

// ---- color / styling ----------------------------------------------------------
// Zero-dependency ANSI, disabled when stdout is not a TTY, on NO_COLOR, or --no-color.

static COLOR: OnceLock<bool> = OnceLock::new();
const BOLD: &str = "1";
const DIM: &str = "2";

type Rgb = (u8, u8, u8);
const WHITE: Rgb = (255, 255, 255);
const BLACK: Rgb = (0, 0, 0);
// Borrowed verbatim from Gerrit's Web UI theme (polygerrit-ui app-theme.ts):
// the --status-* / --vote-color-* variables resolve to these palette hexes.
const GRAY_700: Rgb = (95, 99, 104);   // --status-merged / --status-abandoned / modified
const YELLOW_700: Rgb = (242, 153, 0); // --status-active (black text)
const WIP_BROWN: Rgb = (121, 85, 72);  // --status-wip (#795548)
const PURPLE_500: Rgb = (161, 66, 244);// --status-private / rewrite files
const GREEN_700: Rgb = (24, 128, 56);  // --status-ready / satisfied / additions
const GREEN_300: Rgb = (129, 201, 149);// --vote-color-approved chip bg
const RED_300: Rgb = (242, 139, 130);  // --vote-color-rejected chip bg
const RED_600: Rgb = (217, 48, 37);    // --status-conflict / deletions
const BLUE_700: Rgb = (25, 103, 210);  // links / renamed files
const HEADER_INDIGO: Rgb = (62, 78, 138); // Gerrit top-bar background, used for rules

fn use_color() -> bool {
    *COLOR.get_or_init(|| {
        use std::io::IsTerminal;
        if std::env::var_os("NO_COLOR").is_some() { return false; }
        if std::env::var_os("CLICOLOR_FORCE").is_some() { return true; } // force through a pipe
        std::io::stdout().is_terminal() // colored in iTerm2 et al., plain when piped/redirected
    })
}

/// Wrap in an SGR escape (bold/dim) when color is on.
fn sgr(s: &str, code: &str) -> String {
    if use_color() { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
}

/// 24-bit foreground (iTerm2 & other truecolor terminals).
fn fg(s: &str, (r, g, b): Rgb) -> String {
    if use_color() { format!("\x1b[38;2;{r};{g};{b}m{s}\x1b[0m") } else { s.to_string() }
}

/// A blue link -- repo/branch, Change-Id, SHAs, URLs.
fn link(s: &str) -> String {
    fg(s, BLUE_700)
}

/// 24-bit filled chip: foreground on background.
fn chip(s: &str, (fr, fgc, fb): Rgb, (br, bgc, bb): Rgb) -> String {
    if use_color() {
        format!("\x1b[38;2;{fr};{fgc};{fb};48;2;{br};{bgc};{bb}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn status_badge(ci: &ChangeInfo) -> String {
    // Derive the display state the way gr-change-status does: the raw REST status is
    // only NEW/MERGED/ABANDONED, but an open change reads WIP or Private when those
    // flags are set, otherwise "Active".
    let raw = enum_name(&format!("{:?}", ci.status)).to_uppercase();
    let (label, fgc, bg) = match raw.as_str() {
        "MERGED" => ("Merged", WHITE, GRAY_700),
        "ABANDONED" => ("Abandoned", WHITE, GRAY_700),
        _ if ci.work_in_progress.unwrap_or(false) => ("WIP", WHITE, WIP_BROWN),
        _ if ci.is_private.unwrap_or(false) => ("Private", WHITE, PURPLE_500),
        _ => ("Active", BLACK, YELLOW_700),
    };
    chip(&format!(" {label} "), fgc, bg)
}

fn vote_chip(v: i32, who: &str) -> String {
    // Vote chips: Gerrit's approved/rejected backgrounds with dark text.
    let bg = if v > 0 { GREEN_300 } else { RED_300 };
    format!("{} {who}", chip(&format!(" {v:+} "), BLACK, bg))
}

fn plusminus(ins: i32, del: i32) -> String {
    format!("{} {}", fg(&format!("+{ins}"), GREEN_700), fg(&format!("-{del}"), RED_600))
}

/// (icon, colored status text) for a submit-requirement status.
fn req_parts(status: &str) -> (String, String) {
    let (icon, color) = match status {
        "Satisfied" => ("✓", Some(GREEN_700)),
        "Unsatisfied" => ("✗", Some(RED_600)),
        "NotApplicable" => ("○", None),
        _ => ("·", None),
    };
    match color {
        Some(c) => (fg(icon, c), fg(status, c)),
        None => (sgr(icon, DIM), sgr(status, DIM)),
    }
}

fn file_status(s: Option<&str>) -> (&'static str, Rgb) {
    match s {
        Some("A") => ("A", GREEN_700),  // added
        Some("D") => ("D", RED_600),    // deleted
        Some("R") => ("R", BLUE_700),   // renamed
        Some("C") => ("C", BLUE_700),   // copied
        Some("W") => ("W", PURPLE_500), // rewrite
        _ => ("M", GRAY_700),           // modified
    }
}

//! Demo: post a change message (POST /changes/{id}/revisions/current/review with
//! a ReviewInput, input schema). Mutating -> requires HTTP-basic auth over /a/.
//! Point it at your OWN / a local dev Gerrit, not a shared server.
use gerrit_client::apis::changes_api;
use gerrit_client::apis::configuration::Configuration;
use gerrit_client::models::ReviewInput;

fn main() {
    let mut url = "http://localhost:8080".to_string();
    let mut change = "1".to_string();
    let mut user: Option<String> = None;
    let mut token: Option<String> = None;
    let mut message: Option<String> = None;

    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].clone().as_str() {
            "--url" => { i += 1; if let Some(v) = argv.get(i) { url = v.clone(); } }
            "--change" => { i += 1; if let Some(v) = argv.get(i) { change = v.clone(); } }
            "--user" => { i += 1; user = argv.get(i).cloned(); }
            "--token" => { i += 1; token = argv.get(i).cloned(); }
            "--comment" => { i += 1; message = argv.get(i).cloned(); }
            "-h" | "--help" => {
                eprintln!("usage: post-comment --url <base> --user <name> --token <http-password> \\\n\
                           \x20              --change <id> --comment <text>\n\
                           \n\
                           Token: Gerrit -> Settings -> HTTP Credentials -> Generate new password.");
                return;
            }
            other => { eprintln!("unknown arg: {other}"); std::process::exit(2); }
        }
        i += 1;
    }

    let message = message.unwrap_or_else(|| { eprintln!("error: --comment <text> is required"); std::process::exit(2); });
    let (user, token) = match (user, token) {
        (Some(u), Some(t)) => (u, t),
        _ => { eprintln!("error: --user and --token are required (posting mutates the change)"); std::process::exit(2); }
    };

    let mut cfg = Configuration::new();
    // Standalone REST auth: HTTP basic over the /a/ path.
    cfg.base_path = format!("{}/a", url.trim_end_matches('/'));
    cfg.basic_auth = Some((user, Some(token)));
    cfg.user_agent = Some("gerrit-sdk-rust-client".to_string());

    let mut review = ReviewInput::new();
    review.message = Some(message);

    match changes_api::post_changes_change_id_revisions_revision_id_review(&cfg, &change, "current", Some(review)) {
        Ok(res) => println!("posted comment on change {change}; ready={:?} error={:?}", res.ready, res.error),
        Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
    }
}

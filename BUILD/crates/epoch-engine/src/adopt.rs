//! Setting an agent up in a project, so nobody has to (step 5.2).
//!
//! ## Why this exists
//!
//! Connecting an agent was: copy a block, find the project folder, create `.mcp.json`, paste.
//! Epoch already knows where the project is and what the token is. A setup somebody can get
//! wrong, that Epoch could have done, is a setup Epoch should do.
//!
//! ## What this deliberately does **not** do any more
//!
//! It used to also write a deny list taking away the agent's own `Read`, `Write`, `Edit` and
//! `Bash`, so every file operation had to come through Epoch and be judged by `decide()`.
//!
//! That was wrong, and it was wrong in the most expensive direction: **Claude Code's file tools
//! are better than Epoch's.** Its edits show diffs, its search is smarter, and all of it is far
//! more exercised than anything written here. Taking them away made Epoch worse than using the
//! agent on its own — a toll rather than a door.
//!
//! What it cost was honesty about one sentence. 5.2 was described as *the permission bridge*,
//! and that claim only held while Epoch controlled every file the agent touched. It does not,
//! and it should not. The narrower claim is the true one:
//!
//! > Epoch does not govern the agent's own tools. It governs **its own**, and it says so.
//!
//! The agent already asks the user before it writes — that is not unsafe, it is simply not
//! Epoch's prompt. Pretending otherwise, by showing a `Manual` dropdown that governed nothing,
//! is the decoration this was supposed to prevent.
//!
//! So what Epoch offers an agent is what an agent does **not** have: the crew, the Quest and its
//! Chronicle, the World's knowledge, the MCP servers already configured here, web search — and
//! every one of those still goes through `decide()`, because those genuinely are Epoch's.
//!
//! ## It merges; it never overwrites
//!
//! A project may already have MCP servers and its own permissions. Replacing either would take
//! something away that the user set up on purpose — the same rule Character Pack import follows
//! (ADR-0026: rename on collision, never overwrite), applied to somebody else's file.
//!
//! So: the `epoch` server is added or replaced, every other server is untouched, and the deny
//! list gains what is missing and loses nothing.
//!
//! ## And it says what it did
//!
//! Returns the files it wrote. A tool that edits files in a folder you did not open should be
//! able to name them afterwards.

use serde_json::{json, Value};

use crate::project::ProjectRoot;

#[derive(Debug, thiserror::Error)]
pub enum AdoptError {
    #[error("cannot write {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// What was changed, so a surface can say so.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adopted {
    /// Absolute paths of the files written.
    pub wrote: Vec<String>,
    /// Servers that were already configured and were left alone.
    pub kept: Vec<String>,
    /// Whether `.mcp.json` was added to the project's `.gitignore`, and why not when it was not.
    ///
    /// **Said rather than done quietly.** A line appearing in somebody's `.gitignore` is Epoch
    /// editing their repository, and a repository is not Epoch's to edit without saying so.
    pub ignored: Option<String>,
}

/// Give this project's agent Epoch's tools, on top of its own.
///
/// One file, and only servers. The agent's own permissions are not touched — see the module
/// note: they are its business, and they are good.
pub fn adopt(root: &ProjectRoot, url: &str, token: &str) -> Result<Adopted, AdoptError> {
    let (servers, kept) = with_epoch(&read(root.path().join(".mcp.json")), url, token);
    write(root.path().join(".mcp.json"), &servers)?;

    // Named the way the user knows the folder, not the way the filesystem resolved it.
    //
    // `ProjectRoot` stores both for exactly this reason, and its own comment says why: a
    // canonicalised Windows path carries a `\?\` prefix and is "technically accurate and
    // unrecognisable". Reporting the resolved path put that prefix on screen — the same mistake
    // the type was built to prevent, made by a new caller that reached for `path()` because it
    // was the one doing the writing.
    let tidy = root.as_authored().trim_end_matches(['/', '\\']);
    let shown = format!("{tidy}{}.mcp.json", std::path::MAIN_SEPARATOR);

    Ok(Adopted {
        wrote: vec![shown],
        kept,
        ignored: keep_out_of_git(root.path()),
    })
}

/// Add `.mcp.json` to the project's `.gitignore`, if the project is a repository.
///
/// **The file holds this session's door and its token.** Committed, it outlives the door it
/// describes and travels to everybody who clones — which is the same mistake as writing a token
/// into a global config, one directory in.
///
/// Only where there is already a `.gitignore` or a `.git`: creating a `.gitignore` in a folder
/// that is not a repository would be Epoch leaving litter in somebody's project to solve a
/// problem they do not have.
///
/// Returns what to tell the user, or `None` when there was nothing to do.
fn keep_out_of_git(root: &std::path::Path) -> Option<String> {
    let ignore = root.join(".gitignore");
    if !ignore.exists() && !root.join(".git").exists() {
        return None;
    }

    let existing = std::fs::read_to_string(&ignore).unwrap_or_default();
    // Already covered — by our line or by one of theirs. Either way there is nothing to add,
    // and adding a duplicate would be Epoch editing a file for no reason.
    if existing
        .lines()
        .any(|line| matches!(line.trim(), ".mcp.json" | "/.mcp.json" | "*.json"))
    {
        return None;
    }

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("\n# Epoch writes this file, and it holds a token for this machine's door.\n");
    next.push_str(".mcp.json\n");

    match std::fs::write(&ignore, next) {
        Ok(()) => Some(".mcp.json was added to this project's .gitignore — it holds a token, and a token in a repository outlives the door it describes.".to_owned()),
        // Not a failure of adopting: the agent is connected either way, and the user can be
        // told rather than stopped.
        Err(err) => Some(format!(
            "Could not add .mcp.json to .gitignore ({err}). It holds a token — keep it out of your history."
        )),
    }
}

/// Read a JSON file, or an empty object.
///
/// Unreadable is empty rather than an error — but note what that costs and why it is still
/// right: a malformed `.mcp.json` is replaced rather than repaired. Refusing instead would mean
/// a stray comma in a file Epoch did not write blocks connecting an agent, with no way forward
/// from inside Epoch.
fn read(path: std::path::PathBuf) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| json!({}))
}

fn write(path: std::path::PathBuf, body: &Value) -> Result<(), AdoptError> {
    let text = serde_json::to_string_pretty(body).unwrap_or_default();
    std::fs::write(&path, text).map_err(|source| AdoptError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// Add Epoch to whatever servers are already configured.
fn with_epoch(existing: &Value, url: &str, token: &str) -> (Value, Vec<String>) {
    let mut root = existing.clone();
    if !root.is_object() {
        root = json!({});
    }
    let servers = root
        .as_object_mut()
        .expect("just made an object")
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }

    let kept = servers
        .as_object()
        .map(|map| map.keys().filter(|k| *k != "epoch").cloned().collect())
        .unwrap_or_default();

    servers["epoch"] = json!({
        "type": "http",
        "url": url,
        "headers": { "Authorization": format!("Bearer {token}") },
    });

    (root, kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(std::path::PathBuf);
    impl Dir {
        fn new(tag: &str) -> Self {
            let d = std::env::temp_dir().join(format!("epoch-adopt-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
        fn root(&self) -> ProjectRoot {
            ProjectRoot::open(self.0.to_str().unwrap()).unwrap()
        }
        fn json(&self, rel: &str) -> Value {
            serde_json::from_str(&std::fs::read_to_string(self.0.join(rel)).unwrap()).unwrap()
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn an_untouched_project_gets_the_server_and_nothing_else() {
        let d = Dir::new("fresh");
        let done = adopt(&d.root(), "http://127.0.0.1:8792/mcp", "abc").unwrap();

        assert_eq!(done.wrote.len(), 1);
        // The folder as the user knows it. A canonicalised Windows path would carry a `\?\`
        // prefix, which is accurate and unrecognisable — the exact thing `ProjectRoot` keeps
        // two forms to avoid.
        assert!(!done.wrote[0].contains("?"), "{}", done.wrote[0]);
        assert!(done.wrote[0].ends_with(".mcp.json"));
        let mcp = d.json(".mcp.json");
        assert_eq!(mcp["mcpServers"]["epoch"]["type"], "http");
        assert_eq!(
            mcp["mcpServers"]["epoch"]["headers"]["Authorization"],
            "Bearer abc"
        );
    }

    #[test]
    fn the_agents_own_permissions_are_never_touched() {
        // The correction that matters. An earlier version wrote a deny list taking away the
        // agent's `Read`, `Write`, `Edit` and `Bash` so every file operation came through
        // Epoch — and Epoch's are worse. Adding tools is the whole offer; removing theirs made
        // Epoch a toll rather than a door.
        let d = Dir::new("untouched");
        std::fs::create_dir_all(d.0.join(".claude")).unwrap();
        std::fs::write(d.0.join(".claude/settings.json"), r#"{"model":"opus"}"#).unwrap();

        adopt(&d.root(), "http://127.0.0.1:8792/mcp", "abc").unwrap();

        let settings = d.json(".claude/settings.json");
        assert_eq!(settings["model"], "opus");
        assert!(
            settings.get("permissions").is_none(),
            "nothing was denied on their behalf"
        );
    }

    #[test]
    fn somebody_elses_servers_are_left_alone() {
        // Replacing them would take away something the user set up on purpose — the same rule
        // Character Pack import follows, applied to somebody else's file.
        let d = Dir::new("existing");
        std::fs::write(
            d.0.join(".mcp.json"),
            r#"{"mcpServers":{"playwright":{"command":"npx","args":["-y","@playwright/mcp"]}}}"#,
        )
        .unwrap();

        let done = adopt(&d.root(), "http://127.0.0.1:8792/mcp", "abc").unwrap();

        assert_eq!(done.kept, vec!["playwright"]);
        let mcp = d.json(".mcp.json");
        assert_eq!(mcp["mcpServers"]["playwright"]["command"], "npx");
        assert!(mcp["mcpServers"]["epoch"].is_object());
    }

    #[test]
    fn a_new_token_replaces_the_old_one_rather_than_sitting_beside_it() {
        // Re-adopting after the token changed must leave exactly one, or the agent sends the
        // wrong one and the failure is a 401 nobody can explain.
        let d = Dir::new("token");
        adopt(&d.root(), "http://127.0.0.1:8792/mcp", "old").unwrap();
        adopt(&d.root(), "http://127.0.0.1:8792/mcp", "new").unwrap();

        let mcp = d.json(".mcp.json");
        assert_eq!(
            mcp["mcpServers"]["epoch"]["headers"]["Authorization"],
            "Bearer new"
        );
    }

    #[test]
    fn a_malformed_file_does_not_block_connecting_an_agent() {
        // Replaced rather than repaired, and that is the trade: refusing would mean a stray
        // comma in a file Epoch did not write blocks the whole feature, with no way forward
        // from inside Epoch.
        let d = Dir::new("broken");
        std::fs::write(d.0.join(".mcp.json"), "{ this is not json").unwrap();

        adopt(&d.root(), "http://127.0.0.1:8792/mcp", "abc").unwrap();
        assert!(d.json(".mcp.json")["mcpServers"]["epoch"].is_object());
    }

    #[test]
    fn the_token_is_kept_out_of_a_repository() {
        // `.mcp.json` holds this session's door and its bearer. Committed, it outlives the door
        // and travels to everybody who clones.
        let dir = Dir::new("gitignore");
        std::fs::write(
            dir.0.join(".gitignore"),
            "target/
",
        )
        .unwrap();

        let told = adopt(&dir.root(), "http://127.0.0.1:1/mcp", "secret").unwrap();

        let ignored = std::fs::read_to_string(dir.0.join(".gitignore")).unwrap();
        assert!(ignored.contains("target/"), "their lines survive");
        assert!(ignored.contains(".mcp.json"));
        // **Said, not done quietly.** Editing somebody's repository without telling them is the
        // thing this is careful about, not the editing itself.
        assert!(told.ignored.is_some());
    }

    #[test]
    fn a_project_that_is_not_a_repository_gets_no_litter() {
        // Creating a `.gitignore` in a folder with no git in it would be Epoch solving a
        // problem the user does not have, in their directory.
        let dir = Dir::new("no-git");
        let told = adopt(&dir.root(), "http://127.0.0.1:1/mcp", "secret").unwrap();

        assert!(!dir.0.join(".gitignore").exists());
        assert!(told.ignored.is_none());
    }

    #[test]
    fn a_line_that_already_covers_it_is_left_alone() {
        let dir = Dir::new("already");
        std::fs::write(
            dir.0.join(".gitignore"),
            "node_modules/
.mcp.json
",
        )
        .unwrap();

        let told = adopt(&dir.root(), "http://127.0.0.1:1/mcp", "secret").unwrap();

        let ignored = std::fs::read_to_string(dir.0.join(".gitignore")).unwrap();
        assert_eq!(ignored.matches(".mcp.json").count(), 1, "no duplicate");
        assert!(told.ignored.is_none(), "and nothing to report");
    }
}

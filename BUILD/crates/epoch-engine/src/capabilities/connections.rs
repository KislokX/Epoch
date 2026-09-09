//! Reading a connected server's own documentation.
//!
//! ## The failure this exists for
//!
//! Asked how to reconnect a Spotify server that was refusing to authenticate, a character
//! answered with instructions for connecting apps *inside ChatGPT* — six confident numbered
//! steps, about the wrong product entirely, with links to the wrong help centre.
//!
//! Nothing it had said anything about that server. It knew the tools existed and that they were
//! failing, and it reached for the nearest thing it recognised. That is what a model does with a
//! gap, and the answer is not a sterner instruction: it is to put the real document in reach.
//!
//! The server's own README says exactly how to authenticate it — it is where the `init` wizard
//! and its redirect URI are written down — and the catalogue already names where that README
//! lives. So the Workshop fetches it at install time and this reads it back.
//!
//! ## Why a capability rather than context
//!
//! It could have been a block in every turn. It is a tool instead, for three reasons: a README
//! is thousands of tokens and most turns need none of it; asking for one is a decision the
//! Chronicle should record; and it goes through `decide()` like everything else, so nothing here
//! is a second way of reaching a file.
//!
//! Read-only, and it reads a file Epoch itself wrote into the vault — not the project, not the
//! network. That is why it needs no Project Root and is available in a World between projects.

use std::path::PathBuf;

use epoch_kernel::{Arguments, CapabilityId, Descriptor, Explanation, Parameter, ValueKind};

use crate::capability::{Capability, CapabilityError, Outcome};

/// Most of a README this build will hand to a character in one go.
///
/// A README can be tens of thousands of tokens — larger than a small local model's whole context
/// — and handed over whole it is *dropped* by the Composer, which means the character never sees
/// it and asks again. So it is cut, and the cut says so. The same reasoning as the ceiling on an
/// MCP tool's reply, and the same size.
const MAX: usize = 24 * 1024;

pub fn read_server_docs() -> Descriptor {
    Descriptor::observing(
        id("read_server_docs"),
        "Read a connected MCP server's own documentation — its README, kept when the server was \
         installed. Use it when a server is failing, needs authenticating or set up, or when you \
         are about to tell somebody how to use it. It is what its authors wrote about it; do not \
         answer from memory about a server whose documentation you can read.",
    )
    .taking([Parameter::required(
        "server",
        ValueKind::Text,
        "Which connected server, by the name it is configured under — `playwright`, `spotify`.",
    )])
}

fn id(raw: &str) -> CapabilityId {
    CapabilityId::new(raw).expect("a capability id written here is valid")
}

/// Reads what the Workshop kept, from the vault.
#[derive(Debug, Clone)]
pub struct ReadServerDocs {
    vault: PathBuf,
}

impl ReadServerDocs {
    pub fn new(vault: impl Into<PathBuf>) -> Self {
        Self {
            vault: vault.into(),
        }
    }

    fn wanted(arguments: &Arguments) -> Result<String, CapabilityError> {
        let server = arguments
            .text("server")
            .map_err(CapabilityError::BadArguments)?
            .trim()
            .to_owned();
        if server.is_empty() {
            return Err(CapabilityError::BadArguments(
                "name the server to read about".into(),
            ));
        }
        // A server id is `[a-z0-9_]` and nothing else, so this cannot become a path. Refused
        // rather than sanitised: `..` filtered out is a filter somebody can be cleverer than,
        // and there is no legitimate server whose name needs it.
        if server
            .chars()
            .any(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
        {
            return Err(CapabilityError::Refused(format!(
                "'{server}' is not a server name"
            )));
        }
        Ok(server)
    }
}

impl Capability for ReadServerDocs {
    fn describe(&self) -> Descriptor {
        read_server_docs()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let server = Self::wanted(arguments)?;
        Ok(Explanation::of(
            &descriptor,
            format!("read what '{server}' documents about itself"),
        )
        .expecting("the README kept when that server was installed"))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let server = Self::wanted(arguments)?;
        let at = crate::workshop::docs_path(&self.vault, &server);

        let Ok(body) = std::fs::read_to_string(&at) else {
            // Said plainly, and said *why*, because the two reasons have different fixes: a
            // server added by hand never had a catalogue entry to fetch a README from, and one
            // whose fetch failed can be reinstalled. A character told "no documentation" can say
            // so; one handed nothing invents six steps about the wrong product.
            return Ok(Outcome::told(format!(
                "No documentation is kept for '{server}'. Epoch saves a server's README when it \
                 is installed from the Workshop, so this one was either added by hand or its \
                 README could not be fetched. Say that rather than answering from memory."
            )));
        };

        if body.len() <= MAX {
            return Ok(Outcome::told(format!(
                "Documentation for '{server}', as its authors wrote it:\n\n{body}"
            )));
        }
        let cut: String = body.chars().take(MAX).collect();
        Ok(Outcome::told(format!(
            "Documentation for '{server}', as its authors wrote it. **Cut** — this is the first \
             {} of {} characters:\n\n{cut}",
            cut.chars().count(),
            body.chars().count()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "epoch-docs-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mcp-docs")).unwrap();
        dir
    }

    fn asked(server: &str) -> Arguments {
        Arguments::new().with("server", epoch_kernel::Value::Text(server.into()))
    }

    #[test]
    fn a_character_can_read_what_a_server_documents_about_itself() {
        let dir = vault("read");
        std::fs::write(
            crate::workshop::docs_path(&dir, "spotify"),
            "# Spotify MCP\n\nRun `npx -y spotify-mcp init` to authenticate.",
        )
        .unwrap();

        let outcome = ReadServerDocs::new(&dir).run(&asked("spotify")).unwrap();
        assert!(
            outcome.content.contains("npx -y spotify-mcp init"),
            "{}",
            outcome.content
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_kept_says_so_rather_than_leaving_a_gap_to_fill() {
        // The whole point. Asked how to reconnect a server it had no documentation for, a
        // character produced six confident steps about connecting apps inside ChatGPT — the
        // wrong product, the wrong help centre. A model fills a gap; this makes the gap speak.
        let dir = vault("absent");
        let outcome = ReadServerDocs::new(&dir).run(&asked("spotify")).unwrap();

        assert!(outcome.content.contains("No documentation is kept"));
        assert!(
            outcome
                .content
                .contains("rather than answering from memory"),
            "{}",
            outcome.content
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_name_that_is_not_a_server_name_is_refused_rather_than_cleaned_up() {
        // A server id is `[a-z0-9_]`, so this cannot become a path. Refused rather than
        // sanitised: a filter is something somebody can be cleverer than.
        let dir = vault("traversal");
        for bad in ["../secrets", "..\\secrets", "a/b", "Spotify"] {
            let outcome = ReadServerDocs::new(&dir).run(&asked(bad));
            assert!(
                matches!(outcome, Err(CapabilityError::Refused(_))),
                "{bad} was allowed"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_enormous_readme_is_cut_and_says_it_was_cut() {
        // Handed over whole it is larger than a small model's context, so the Composer drops it
        // and the character never sees it — then asks again. The same reasoning as the ceiling
        // on an MCP tool's reply.
        let dir = vault("huge");
        std::fs::write(
            crate::workshop::docs_path(&dir, "playwright"),
            "x".repeat(MAX * 2),
        )
        .unwrap();

        let outcome = ReadServerDocs::new(&dir).run(&asked("playwright")).unwrap();
        assert!(
            outcome.content.contains("**Cut**"),
            "a truncated answer must say so"
        );
        assert!(outcome.content.len() < MAX * 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn it_is_an_observation_and_needs_nowhere_to_work() {
        // It reads a file Epoch wrote into its own vault — not the project, not the network —
        // which is why it is available in a World between projects.
        let descriptor = read_server_docs();
        assert!(!descriptor.reversal.is_permanent());
        assert_eq!(
            descriptor.effects,
            std::collections::BTreeSet::from([epoch_kernel::Effect::Reads])
        );
        assert_eq!(descriptor.id.as_str(), "read_server_docs");
    }
}

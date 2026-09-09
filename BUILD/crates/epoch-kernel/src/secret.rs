//! A value that must never leave the machine it was typed on.
//!
//! ## The whole point is what this type *cannot* do
//!
//! `Secret` has no `Serialize` and no `Deserialize`. That is not an omission — it is the
//! guarantee. ADR-0026 says a Character Pack must be *structurally incapable* of containing an
//! API key, and this is what makes that a promise the compiler keeps rather than a rule a
//! reviewer has to notice. Put a `Secret` in a struct that derives `Serialize` and the build
//! stops; there is no path where a key rides along inside somebody's shared character file,
//! their World export, or a projection to the interface.
//!
//! It also has no `Display`, so it cannot be interpolated into a message, a URL or a log line by
//! accident. `Debug` exists, because a type you cannot put in a `dbg!` gets replaced by a
//! `String` the first time somebody is debugging at midnight — and it prints nothing.
//!
//! ## Getting the value out is deliberately awkward
//!
//! `expose()` is named to be uncomfortable to read at a call site that should not have it. There
//! should be very few: one where the key is written to the store, and one per Provider that
//! actually sends it.
//!
//! ## What the rest of the system passes around instead
//!
//! [`SecretName`] — *which* secret, never the secret. That is an ordinary serialisable value,
//! safe in files and safe on the wire, and it is what a configuration refers to.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A credential. Never serialised, never displayed, never logged.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Read the value. Every call site is a place a credential leaves this type — keep them few
    /// and keep them obvious.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// A key somebody cleared is not a key. Checked without exposing anything.
    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Debug for Secret {
    /// Prints nothing about the value — not even its length, which narrows a guess.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(hidden)")
    }
}

/// *Which* secret, never the secret.
///
/// Safe everywhere a `Secret` is not: this is what a configuration file, a projection and an
/// interface refer to. A name that points at nothing is an ordinary state — it means no key has
/// been stored yet, which is different from a key that is empty.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretName(String);

impl SecretName {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The name of the key belonging to one backend.
    ///
    /// Namespaced so that backends, and later characters or MCP servers, cannot collide on a
    /// bare id — `ollama` the backend and `ollama` anything-else are different secrets.
    pub fn for_backend(id: &str) -> Self {
        Self(format!("backend:{id}"))
    }

    /// The bearer credential for Epoch's local MCP door.
    pub fn for_mcp_door() -> Self {
        Self("mcp:door".to_owned())
    }

    /// One environment value an installed MCP server needs kept out of the vault.
    ///
    /// Namespaced by server, so two servers that both want `API_KEY` hold two secrets. Which
    /// values arrive here is decided by the catalogue's own `isSecret`, never by whoever wrote
    /// the install path — the credential is then structurally incapable of reaching the TOML,
    /// which is the same guarantee ADR-0026 gives a Character Pack.
    pub fn for_mcp_input(server: &str, variable: &str) -> Self {
        Self(format!("mcp:{server}/{variable}"))
    }

    /// The key one asset catalogue needs before it will hand a file over.
    ///
    /// Namespaced by source, because two sites are two accounts and one of them being signed in
    /// says nothing about the other. Measured, 2026-08-23: Civitai answers `401` to an anonymous
    /// download and `200` to an anonymous *search*, so this gates half a source rather than all
    /// of it — a shelf is fully usable before anybody signs in anywhere.
    pub fn for_catalogue(source: &str) -> Self {
        Self(format!("catalogue:{source}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_says_nothing_about_itself_when_printed() {
        // Not even the length: that narrows a guess, and a debug line outlives the session it
        // was written in.
        let secret = Secret::new("sk-live-0123456789");
        let printed = format!("{secret:?}");
        assert_eq!(printed, "Secret(hidden)");
        assert!(!printed.contains("sk-"));
        assert!(!printed.contains("18"));
    }

    #[test]
    fn only_the_name_is_serialisable() {
        // The value has no `Serialize` at all, so this is the only half that can be written
        // down. That absence is the guarantee ADR-0026 asks for, and it is enforced by the
        // build rather than by review — a struct deriving Serialize around a Secret does not
        // compile, which is why there is no test that could be written here for the other half.
        let name = SecretName::for_backend("laptop");
        assert_eq!(serde_json::to_string(&name).unwrap(), "\"backend:laptop\"");
    }

    #[test]
    fn a_cleared_key_is_not_a_key() {
        assert!(Secret::new("   ").is_empty());
        assert!(!Secret::new("sk-1").is_empty());
    }
}

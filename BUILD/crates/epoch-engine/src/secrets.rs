//! Where credentials live, and why they cannot live anywhere else.
//!
//! ## The type does half the work; this does the other half
//!
//! [`Secret`] guarantees a key cannot be *written down* by accident — no `Serialize`, so a
//! Character Pack containing one does not compile (ADR-0026). This module answers the question
//! that leaves open: where does a key the user genuinely wants kept actually go?
//!
//! Not `providers.toml`. That file is meant to be read, edited by hand, and eventually shared —
//! a key in it would be a key in every screenshot and every support paste. The store is a
//! separate, opaque file that nothing else reads.
//!
//! ## Encrypted by the operating system, to the user account
//!
//! Windows DPAPI (`CryptProtectData`). The key material belongs to the logged-in account and
//! never exists in Epoch: another account on the same machine cannot read the file, and neither
//! can the file copied to a different machine. That last part is a *feature* — a credential that
//! survived being copied somewhere else would be a credential that leaks with a backup.
//!
//! It is worth being exact about what this does **not** do: an attacker already running code as
//! this user can decrypt it, because Windows will decrypt it for anything that user runs. Every
//! password manager on this platform has that same ceiling. What it removes is the ordinary
//! leak — plaintext on disk, in a sync folder, in a repository, in a backup.
//!
//! ## Nothing is cached
//!
//! Read, decrypt, use, drop. The plaintext exists for the length of one call. Holding a
//! decrypted map for the life of the process would put every key in a memory dump of a crash
//! report, which is the sort of thing that only becomes visible after it has already happened.
//!
//! ## Not on Windows
//!
//! Storing refuses, in words, rather than falling back to a plain file. A store that silently
//! degrades to plaintext is worse than no store: the user believes the first thing they were
//! told. Keychain and Secret Service go here when Epoch ships on those platforms.

use std::path::Path;

use epoch_kernel::{Secret, SecretName};

/// The store, addressed by where the vault is.
///
/// **A name over `epoch-secrets`, not a second implementation.** The sealing, the file and the
/// three operating systems live in that crate because EpochServices needs the same store on
/// machines that are as likely to be a Mac as a PC (ADR-0029 §9). What stays here is the Kernel's
/// vocabulary: a [`SecretName`] rather than a string, and a [`Secret`] that cannot be written
/// down by accident (ADR-0026).
#[derive(Debug, Clone)]
pub struct Secrets {
    inner: epoch_secrets::Store,
}

impl Secrets {
    pub fn at(vault: &Path) -> Self {
        Self {
            inner: epoch_secrets::Store::at(vault),
        }
    }

    /// The vault this store belongs to.
    pub fn vault(&self) -> &Path {
        self.inner.vault()
    }

    /// Whether this machine's credential store can be written to right now, and if not, which
    /// kind of not. See [`epoch_secrets::Reach`] — *locked* is nobody at the machine, which is
    /// a different answer from *broken*.
    pub fn reachable(&self) -> epoch_secrets::Reach {
        self.inner.reachable()
    }

    /// Read one. `None` covers every reason equally on purpose — absent, unreadable, belonging
    /// to another account — because the caller's next move is the same in all of them, and a
    /// caller that could distinguish them could probe the store.
    pub fn get(&self, name: &SecretName) -> Option<Secret> {
        self.inner.get(name.as_str()).map(Secret::new)
    }

    /// Read one while preserving the difference between a missing store and an unreadable one.
    /// Security migrations use this to avoid overwriting unrelated credentials on corruption.
    pub fn get_checked(&self, name: &SecretName) -> Result<Option<Secret>, String> {
        Ok(self.inner.get_checked(name.as_str())?.map(Secret::new))
    }

    /// Whether a key exists.
    pub fn holds(&self, name: &SecretName) -> bool {
        self.inner.holds(name.as_str())
    }

    /// Every name that has a value, for a surface that must say *stored* or *none*.
    pub fn names(&self) -> Vec<SecretName> {
        self.inner
            .names()
            .into_iter()
            .map(SecretName::new)
            .collect()
    }

    /// Store one, replacing whatever was there.
    ///
    /// An empty value forgets instead of storing emptiness: somebody who cleared the field meant
    /// to remove the key, and a stored empty string would read as *configured* everywhere.
    pub fn put(&self, name: &SecretName, secret: &Secret) -> Result<(), String> {
        self.inner.put(name.as_str(), secret.expose())
    }

    /// Remove one. Removing what was never there is success, not an error.
    pub fn forget(&self, name: &SecretName) -> Result<(), String> {
        self.inner.forget(name.as_str())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    struct Dir(std::path::PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("epoch-secrets-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const KEY: &str = "sk-live-do-not-log-this";

    #[test]
    fn a_key_survives_the_round_trip() {
        let d = Dir::new("roundtrip");
        let store = Secrets::at(&d.0);
        let name = SecretName::for_backend("desktop");

        assert!(
            !store.holds(&name),
            "nothing is kept before anything was put"
        );
        store.put(&name, &Secret::new(KEY)).unwrap();

        assert!(store.holds(&name));
        assert_eq!(store.get(&name).unwrap().expose(), KEY);
        assert_eq!(store.names(), vec![name]);
    }

    #[test]
    fn the_file_never_contains_the_key() {
        // The whole reason the store is separate from providers.toml. If this ever fails, a
        // credential is one screenshot away from being public.
        let d = Dir::new("opaque");
        let store = Secrets::at(&d.0);
        store
            .put(&SecretName::for_backend("laptop"), &Secret::new(KEY))
            .unwrap();

        let raw = std::fs::read(d.0.join("secrets.dat")).unwrap();
        assert!(
            raw.windows(KEY.len()).all(|w| w != KEY.as_bytes()),
            "the plaintext key is on disk"
        );
        // Nor the name of what it belongs to: the file must not advertise which services
        // somebody uses.
        assert!(raw.windows(6).all(|w| w != b"laptop"));
    }

    #[test]
    fn keys_do_not_disturb_each_other() {
        let d = Dir::new("several");
        let store = Secrets::at(&d.0);
        let one = SecretName::for_backend("one");
        let two = SecretName::for_backend("two");

        store.put(&one, &Secret::new("first")).unwrap();
        store.put(&two, &Secret::new("second")).unwrap();
        store.forget(&one).unwrap();

        assert!(!store.holds(&one));
        assert_eq!(
            store.get(&two).unwrap().expose(),
            "second",
            "the other survived"
        );
    }

    #[test]
    fn clearing_the_field_removes_the_key_rather_than_storing_emptiness() {
        // A stored empty string would read as *configured* everywhere that asks.
        let d = Dir::new("cleared");
        let store = Secrets::at(&d.0);
        let name = SecretName::for_backend("laptop");

        store.put(&name, &Secret::new(KEY)).unwrap();
        store.put(&name, &Secret::new("   ")).unwrap();
        assert!(!store.holds(&name));
    }

    #[test]
    fn the_last_key_takes_the_store_with_it() {
        // An encrypted blob of `{}` left behind says something is kept when nothing is.
        let d = Dir::new("last");
        let store = Secrets::at(&d.0);
        let name = SecretName::for_backend("only");

        store.put(&name, &Secret::new(KEY)).unwrap();
        store.forget(&name).unwrap();
        assert!(!d.0.join("secrets.dat").exists());
        // And forgetting what was never there is success: the caller wanted it gone.
        assert!(store.forget(&name).is_ok());
    }

    #[test]
    fn a_store_from_somewhere_else_is_refused_rather_than_guessed_at() {
        // Copied from another account or another machine. DPAPI cannot open it, and that is the
        // design — a credential that survived being copied would leak with a backup.
        let d = Dir::new("foreign");
        let store = Secrets::at(&d.0);
        std::fs::write(
            d.0.join("secrets.dat"),
            b"not a blob this account ever sealed",
        )
        .unwrap();

        assert!(store.get(&SecretName::for_backend("laptop")).is_none());
        assert!(store.names().is_empty());
        // And an unreadable store is never rewritten from scratch to satisfy one deletion.
        store.forget(&SecretName::for_backend("laptop")).unwrap();
        assert!(
            d.0.join("secrets.dat").exists(),
            "somebody else's file is left alone"
        );

        // Saving a different credential must not turn corruption into permission to erase every
        // existing encrypted value. The caller gets a named failure and can repair the account
        // store deliberately.
        assert!(store
            .put(&SecretName::for_backend("new"), &Secret::new("replacement"))
            .is_err());
        assert_eq!(
            std::fs::read(d.0.join("secrets.dat")).unwrap(),
            b"not a blob this account ever sealed"
        );
    }
}

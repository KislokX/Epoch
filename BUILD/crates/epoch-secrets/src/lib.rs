//! Where a credential is kept, on whichever operating system this is.
//!
//! ## Two programs, one answer
//!
//! Epoch keeps provider keys, agent door tokens and catalogue keys. EpochServices needs the same,
//! for the same reason and on machines that are as likely to be a Mac as a PC. A second
//! implementation would be a second answer to *where did my key go*, and the two would drift the
//! first time one of them gained a platform.
//!
//! ## The operating system holds it, and the three of them hold it differently
//!
//! | | where | by what |
//! |---|---|---|
//! | Windows | an opaque file beside the vault | DPAPI, sealed to this account |
//! | macOS | the login keychain | one generic-password item |
//! | Linux | the Secret Service | one `secret-tool` item |
//!
//! **Windows keeps a file and the other two do not**, and that is the platforms' own shape
//! rather than a compromise: DPAPI seals bytes and hands them back, so somebody has to store the
//! bytes; Keychain and Secret Service *are* the store. Pretending all three had a file would mean
//! writing a plaintext file on two of them.
//!
//! ## It refuses rather than degrading
//!
//! A store that quietly fell back to a plain file would be worse than none: the user believes the
//! first thing they were told. Every failure here says what it could not do.
//!
//! ## Nothing is cached
//!
//! Read, decrypt, use, drop. Holding a decrypted map for the life of the process would put every
//! key into a memory dump of a crash report — the sort of thing that only becomes visible after
//! it has already happened.
//!
//! ## Measured where it could be
//!
//! The Windows path is exercised by this machine's tests and by years of use. **The macOS and
//! Linux paths are written and unmeasured** — `security` and `secret-tool` are called exactly as
//! their manuals describe, and neither has been run here. `what_is_here` is the ignored test that
//! settles it on a machine that has them, and until somebody runs it this note is the honest
//! statement of what is known.

/// Writing a file that must never be found half-written.
///
/// Beside the credential store because it answers the same question: where does something go
/// that nobody can get back?
pub mod atomically;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What the store holds once opened: a name, and what it is worth.
type Kept = BTreeMap<String, String>;

/// The credential store belonging to one vault.
///
/// Holds no state: a path and a set of operations. Everything else is in the operating system, or
/// in a local variable for a few microseconds.
#[derive(Debug, Clone)]
pub struct Store {
    /// Where the vault is. On Windows the file lives here; elsewhere this only names *which*
    /// store, so two vaults on one machine do not share a keychain item.
    vault: PathBuf,
}

impl Store {
    pub fn at(vault: impl Into<PathBuf>) -> Self {
        Self {
            vault: vault.into(),
        }
    }

    pub fn vault(&self) -> &Path {
        &self.vault
    }

    /// Read one.
    ///
    /// `None` covers every reason equally on purpose — absent, unreadable, belonging to another
    /// account — because the caller's next move is the same in all of them, and a caller that
    /// could tell them apart could probe the store.
    pub fn get(&self, name: &str) -> Option<String> {
        self.get_checked(name).ok().flatten()
    }

    /// Read one while keeping the difference between a missing store and an unreadable one.
    pub fn get_checked(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self
            .read_optional()?
            .and_then(|kept| kept.get(name).cloned()))
    }

    /// Whether a name has a value.
    ///
    /// Still opens the store: the names live *inside* it rather than in filenames, so that
    /// nothing on disk advertises which services somebody uses.
    pub fn holds(&self, name: &str) -> bool {
        self.read()
            .map(|kept| kept.contains_key(name))
            .unwrap_or(false)
    }

    /// Every name that has a value, for a surface that must say *stored* or *none*.
    pub fn names(&self) -> Vec<String> {
        self.read()
            .map(|kept| kept.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether this machine's credential store can be written to **right now**.
    ///
    /// ## Why the three answers are not two
    ///
    /// A store that refuses is not always a store that is broken. On macOS the login keychain is
    /// locked until somebody signs in *at the machine*, so `security` answers `User interaction
    /// is not allowed` over SSH, in a service, or on a build agent with no session — and that is
    /// the machine being asked at the wrong moment rather than anything being wrong.
    ///
    /// Collapsing that into `false` would be the inversion this codebase keeps paying for: it
    /// reads *nobody could ask* as *no*. Collapsing it into `true` would be worse. So there are
    /// three answers, and a caller decides what each one means for it.
    ///
    /// **It writes.** A store that answers questions about itself without touching the operating
    /// system is answering about a different store; this puts one throwaway value in and takes it
    /// out again. Not for a hot path.
    pub fn reachable(&self) -> Reach {
        const PROBE: &str = "epoch:can-this-be-written";
        match self.put(PROBE, "yes") {
            Ok(()) => {
                let _ = self.forget(PROBE);
                Reach::Yes
            }
            Err(why) if locked(&why) => Reach::Locked(why),
            Err(why) => Reach::Broken(why),
        }
    }

    /// Store one, replacing whatever was there.
    ///
    /// An empty value forgets rather than storing emptiness: somebody who cleared the field meant
    /// to remove the key, and a stored empty string reads as *configured* everywhere.
    pub fn put(&self, name: &str, value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            return self.forget(name);
        }
        let mut kept = self.read_optional()?.unwrap_or_default();
        kept.insert(name.to_owned(), value.to_owned());
        self.write(&kept)
    }

    /// Remove one. Removing what was never there is success: the caller wanted it gone, it is
    /// gone.
    pub fn forget(&self, name: &str) -> Result<(), String> {
        let Ok(mut kept) = self.read() else {
            // An unreadable store has nothing this name can be removed from, and rewriting it
            // from scratch here would destroy every other key to satisfy one deletion.
            return Ok(());
        };
        if kept.remove(name).is_none() {
            return Ok(());
        }
        if kept.is_empty() {
            // An empty store is no store. Leaving one behind would say something is kept when
            // nothing is.
            return self.erase();
        }
        self.write(&kept)
    }

    fn read(&self) -> Result<Kept, String> {
        self.read_optional()?
            .ok_or_else(|| "there is no credential store yet".to_owned())
    }

    fn read_optional(&self) -> Result<Option<Kept>, String> {
        let Some(plain) = self.load()? else {
            return Ok(None);
        };
        // JSON is the format *inside* the envelope only. It never touches a disk unsealed.
        serde_json::from_slice(&plain)
            .map(Some)
            .map_err(|why| why.to_string())
    }

    fn write(&self, kept: &Kept) -> Result<(), String> {
        let plain = serde_json::to_vec(kept).map_err(|why| why.to_string())?;
        self.save(&plain)
    }
}

// ---------------------------------------------------------------------------------- Windows

#[cfg(windows)]
impl Store {
    fn file(&self) -> PathBuf {
        self.vault.join("secrets.dat")
    }

    fn load(&self) -> Result<Option<Vec<u8>>, String> {
        match std::fs::read(self.file()) {
            Ok(sealed) => dpapi(&sealed, false).map(Some),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(why) => Err(why.to_string()),
        }
    }

    fn save(&self, plain: &[u8]) -> Result<(), String> {
        let sealed = dpapi(plain, true)?;
        if let Some(parent) = self.file().parent() {
            std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;
        }
        // **Atomically**: a torn write here is every provider key, every agent door token and
        // every machine's bearer, and none of them can be re-derived from anything.
        atomically::replace(&self.file(), &sealed)
    }

    fn erase(&self) -> Result<(), String> {
        std::fs::remove_file(self.file()).map_err(|why| why.to_string())
    }
}

/// Extra entropy mixed into every blob.
///
/// Honest about what this is: the constant is inside the binary, so it stops another program on
/// the same account from decrypting this file by simply calling `CryptUnprotectData` on it. It
/// stops nothing at all against somebody holding the binary. That is the standard bargain and it
/// is worth the four lines, but it is not a second layer of security.
#[cfg(windows)]
const ENTROPY: &[u8] = b"epoch.secrets.v1";

/// The one place unsafe lives.
///
/// Both directions are the same shape — two in-blobs, one out-blob, and a buffer Windows
/// allocated that must go back through `LocalFree`. Writing them separately duplicated the
/// freeing, and the freeing is the half that is wrong silently.
#[cfg(windows)]
fn dpapi(input: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };

    // `pbData` is `*mut` in the API and read-only in fact for the input blobs: DPAPI does not
    // write through them. The casts are confined to these two lines.
    let data = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let ok = unsafe {
        if protect {
            CryptProtectData(&data, null(), &entropy, null(), null(), 0, &mut out)
        } else {
            CryptUnprotectData(&data, null_mut(), &entropy, null(), null(), 0, &mut out)
        }
    };

    if ok == 0 {
        // Deliberately not the OS error code. The common failure is a file belonging to another
        // account or another machine, and `error 13` explains nothing to the person reading it.
        return Err(if protect {
            "Windows would not encrypt the credential store.".to_owned()
        } else {
            "The credential store could not be read. It belongs to a different Windows account \
             or a different machine, and it cannot be moved between them by design."
                .to_owned()
        });
    }

    // Copy out before freeing: what Windows returned is its allocation, not ours.
    let copied = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
    unsafe { LocalFree(out.pbData as *mut core::ffi::c_void) };
    Ok(copied)
}

// ------------------------------------------------------------------------------------ macOS

/// Which keychain item belongs to this vault.
///
/// The path is folded into the account name so two vaults on one machine do not share an item —
/// a keychain is per-user, not per-folder, and one item for both would mean one vault's key
/// silently replacing the other's.
#[cfg(not(windows))]
fn account_for(vault: &Path) -> String {
    // Not a cryptographic hash and not required to be: it distinguishes folders, and a collision
    // would mean two vaults sharing a store rather than anything leaking.
    let mut mixed: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in vault.to_string_lossy().as_bytes() {
        mixed ^= u64::from(*byte);
        mixed = mixed.wrapping_mul(0x100_0000_01b3);
    }
    format!("vault-{mixed:016x}")
}

#[cfg(target_os = "macos")]
impl Store {
    fn load(&self) -> Result<Option<Vec<u8>>, String> {
        // `-w` prints the password and nothing else. A missing item exits non-zero, which is an
        // ordinary state rather than a failure.
        let out = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                SERVICE,
                "-a",
                &account_for(&self.vault),
                "-w",
            ])
            .output()
            .map_err(|why| format!("macOS `security` could not be run: {why}"))?;
        if !out.status.success() {
            return Ok(None);
        }
        Ok(Some(out.stdout.trim_ascii_end().to_vec()))
    }

    /// Store it, **without the value ever appearing in an argument**.
    ///
    /// The first version passed `-w <value>`, and `security` says of itself:
    ///
    /// ```text
    /// Use of the -p or -w options is insecure. Specify -w as the last option to be prompted.
    /// ```
    ///
    /// Which is the rule the Linux path already followed and this one did not: an argument is
    /// visible in the process list to every user on the machine, so an API key sat there for as
    /// long as the call took. Written on one platform, forgotten on the other.
    ///
    /// **Measured 2026-08-24 on macOS 26.6.2 (arm64), in a throwaway keychain**, because the
    /// manual's word *prompted* does not say whether a pipe counts. It does — and it asks
    /// **twice**:
    ///
    /// ```text
    /// password data for new item: retype password for new item: exit=0
    /// ```
    ///
    /// So the value is fed twice and the exchange is silent. `-w` must be **last**; with a
    /// keychain named after it the command is a usage error, which is why none is named (Epoch
    /// never named one anyway — the default keychain is the right one).
    fn save(&self, plain: &[u8]) -> Result<(), String> {
        use std::io::Write as _;

        let value = String::from_utf8(plain.to_vec()).map_err(|why| why.to_string())?;
        // **A line at a time is enough, and that is a property of the caller, not luck.**
        // What reaches `save` is always one compact `serde_json::to_vec` document — a newline
        // inside a stored value is escaped to two characters *within* that line and never
        // appears as one.
        // Measured on macOS 2026-08-24: a value containing a newline stored and read back
        // unchanged. A guard that refused one was written first and deleted after the test
        // proved it could never fire — the interesting fact is the invariant, not a check for
        // something that cannot happen.
        // `-U` updates in place rather than refusing because the item exists.
        let mut child = std::process::Command::new("security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                SERVICE,
                "-a",
                &account_for(&self.vault),
                "-w",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|why| format!("macOS `security` could not be run: {why}"))?;
        child
            .stdin
            .as_mut()
            .ok_or("macOS `security` would not take the value")?
            .write_all(format!("{value}\n{value}\n").as_bytes())
            .map_err(|why| why.to_string())?;
        let out = child
            .wait_with_output()
            .map_err(|why| format!("macOS `security` could not be run: {why}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "the macOS keychain refused it: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    fn erase(&self) -> Result<(), String> {
        let _ = std::process::Command::new("security")
            .args([
                "delete-generic-password",
                "-s",
                SERVICE,
                "-a",
                &account_for(&self.vault),
            ])
            .output();
        Ok(())
    }
}

// ------------------------------------------------------------------------------------ Linux

#[cfg(all(unix, not(target_os = "macos")))]
impl Store {
    fn load(&self) -> Result<Option<Vec<u8>>, String> {
        let out = std::process::Command::new("secret-tool")
            .args([
                "lookup",
                "service",
                SERVICE,
                "account",
                &account_for(&self.vault),
            ])
            .output()
            .map_err(|why| {
                format!("`secret-tool` could not be run ({why}). It comes with libsecret.")
            })?;
        if !out.status.success() || out.stdout.is_empty() {
            return Ok(None);
        }
        Ok(Some(out.stdout))
    }

    fn save(&self, plain: &[u8]) -> Result<(), String> {
        use std::io::Write as _;

        // The value goes in on stdin rather than as an argument: an argument is visible in the
        // process list to every user on the machine.
        let mut child = std::process::Command::new("secret-tool")
            .args([
                "store",
                "--label=Epoch",
                "service",
                SERVICE,
                "account",
                &account_for(&self.vault),
            ])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|why| {
                format!("`secret-tool` could not be run ({why}). It comes with libsecret.")
            })?;
        child
            .stdin
            .as_mut()
            .ok_or("`secret-tool` would not take the value")?
            .write_all(plain)
            .map_err(|why| why.to_string())?;
        let done = child.wait().map_err(|why| why.to_string())?;
        if done.success() {
            Ok(())
        } else {
            Err("the Secret Service refused it. Is a keyring unlocked?".to_owned())
        }
    }

    fn erase(&self) -> Result<(), String> {
        let _ = std::process::Command::new("secret-tool")
            .args([
                "clear",
                "service",
                SERVICE,
                "account",
                &account_for(&self.vault),
            ])
            .output();
        Ok(())
    }
}

/// Whether the credential store can be written to, and if not, which kind of not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// It works.
    Yes,
    /// It exists and nobody is signed in at the machine to unlock it. Asking again from a
    /// desktop session is the whole fix.
    Locked(String),
    /// Something else, in the operating system's own words.
    Broken(String),
}

/// Does this refusal mean *nobody is at the machine* rather than *this is broken*?
///
/// Read from what each platform actually says. macOS is `errSecInteractionNotAllowed`, which
/// `security` prints in words; a Linux box with no desktop session has no secret service to talk
/// to, and D-Bus says so. Windows is absent from the list because DPAPI has no locked state — it
/// decrypts for the account that is already running the process.
fn locked(why: &str) -> bool {
    let said = why.to_ascii_lowercase();
    said.contains("user interaction is not allowed")
        || said.contains("interactionnotallowed")
        || said.contains("cannot autolaunch d-bus")
        || said.contains("no such interface")
}

/// What this store calls itself to the operating system.
#[cfg(not(windows))]
const SERVICE: &str = "Epoch";

#[cfg(test)]
mod tests {
    use super::*;

    /// A store to test against, or the reason this machine cannot be asked.
    ///
    /// **Only [`Reach::Locked`] is a reason to stop.** A locked keychain is nobody at the
    /// machine — true over SSH, in a service, on an agent with no session — and skipping there
    /// keeps a developer's remote shell from reporting a fault that does not exist. A store that
    /// is *broken* still fails, loudly, which is the half that must not be lost: four tests that
    /// skip on any refusal are four tests that go green the day the store stops working.
    fn store_at(dir: &Dir) -> Option<Store> {
        let store = Store::at(&dir.0);
        match store.reachable() {
            Reach::Yes => Some(store),
            Reach::Locked(why) => {
                eprintln!(
                    "skipped: this machine's credential store is locked, so nothing can be                      written to it from here — {why}"
                );
                None
            }
            Reach::Broken(why) => panic!("the credential store is not working: {why}"),
        }
    }

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("epoch-secrets-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = Store::at(&self.0).forget("a");
            let _ = Store::at(&self.0).forget("catalogue:civitai");
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_store_nobody_has_written_to_holds_nothing() {
        // The difference between *no store* and *an empty one*: asking must not create anything.
        let dir = Dir::new("empty");
        let Some(store) = store_at(&dir) else { return };
        assert_eq!(store.get("a"), None);
        assert!(!store.holds("a"));
        assert!(store.names().is_empty());
    }

    #[test]
    fn what_goes_in_comes_back_and_nothing_else_does() {
        let dir = Dir::new("roundtrip");
        let Some(store) = store_at(&dir) else { return };
        store.put("a", "one").unwrap();
        store.put("b", "two").unwrap();
        assert_eq!(store.get("a").as_deref(), Some("one"));
        assert_eq!(store.get("b").as_deref(), Some("two"));
        assert_eq!(store.get("c"), None);
        assert_eq!(store.names(), vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn clearing_a_field_removes_the_key_rather_than_storing_emptiness() {
        // A stored empty string reads as *configured* on every screen that asks.
        let dir = Dir::new("cleared");
        let Some(store) = store_at(&dir) else { return };
        store.put("a", "one").unwrap();
        store.put("a", "   ").unwrap();
        assert!(!store.holds("a"));
    }

    #[test]
    fn forgetting_one_leaves_the_others_alone() {
        let dir = Dir::new("forget");
        let Some(store) = store_at(&dir) else { return };
        store.put("a", "one").unwrap();
        store.put("b", "two").unwrap();
        store.forget("a").unwrap();
        assert!(!store.holds("a"));
        assert_eq!(store.get("b").as_deref(), Some("two"));
        // And removing what was never there is success: the caller wanted it gone.
        store.forget("nothing-like-this").unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn nothing_readable_is_written_to_disk() {
        let dir = Dir::new("opaque");
        let store = Store::at(&dir.0);
        store
            .put("catalogue:civitai", "a-very-recognisable-secret")
            .unwrap();
        let on_disk = String::from_utf8_lossy(&std::fs::read(dir.0.join("secrets.dat")).unwrap())
            .into_owned();
        assert!(
            !on_disk.contains("a-very-recognisable-secret"),
            "the credential is on disk in the clear"
        );
        // And the *name* is inside the envelope too, so the file does not advertise which
        // services somebody uses. A single letter would appear in any ciphertext by chance,
        // which is why this looks for a whole name.
        assert!(
            !on_disk.contains("catalogue:civitai"),
            "the name is readable"
        );
    }

    #[test]
    #[cfg(not(windows))]
    fn two_vaults_do_not_share_one_keychain_item() {
        // A keychain is per-user, not per-folder. One item for both would mean one vault's key
        // silently replacing the other's.
        assert_ne!(
            account_for(Path::new("/one/vault")),
            account_for(Path::new("/another/vault"))
        );
    }

    /// A value containing a newline survives the round trip, on every platform.
    ///
    /// **The macOS path hands the value to `security` a line at a time**, so this looks like the
    /// one thing that would break there. It does not, and the reason is worth holding: a stored
    /// value never reaches the keychain on its own — it reaches it inside one compact JSON
    /// document, where a newline is two characters.
    ///
    /// Measured on macOS 26.6.2 (arm64) 2026-08-24, against a real keychain. A guard refusing
    /// such a value existed for about an hour, until this test showed it could never fire.
    #[test]
    #[ignore = "writes into this machine's credential store"]
    fn a_value_with_a_line_break_survives_the_round_trip() {
        let dir = Dir::new("linebreak");
        let store = Store::at(&dir.0);
        store
            .put(
                "a",
                "first
second",
            )
            .expect("stored");
        assert_eq!(
            store.get("a").as_deref(),
            Some(
                "first
second"
            )
        );
        store.forget("a").unwrap();
    }

    /// What this machine's own store really does.
    ///
    /// Ignored because it writes into the operating system's keychain. Run deliberately, it is
    /// the only thing that settles whether the macOS and Linux paths work — they are written
    /// from their manuals and, on the machine this was built on, unmeasured.
    #[test]
    #[ignore = "writes into this machine's credential store"]
    fn what_is_here() {
        let dir = Dir::new("measured");
        let store = Store::at(&dir.0);
        println!("vault: {}", store.vault().display());
        match store.put("a", "measured") {
            Ok(()) => println!("stored"),
            Err(why) => panic!("could not store: {why}"),
        }
        assert_eq!(store.get("a").as_deref(), Some("measured"));
        store.forget("a").unwrap();
        assert_eq!(store.get("a"), None);
        println!("read back and removed");
    }
}

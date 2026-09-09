//! Who this machine is, on a network with no authority to ask.
//!
//! ## Why a certificate at all
//!
//! Everything a Host and a lent machine say to each other used to cross the network in clear
//! text: the composed turn — the whole Chronicle, the system prompt, the project's context —
//! and, at pairing, the long secret itself. Anything on that network could read a conversation
//! and, once, take the credential that lets it pretend to be the Host forever.
//!
//! ## Why it is self-signed, and what replaces the authority
//!
//! There is nobody to ask. A machine on somebody's home network has no name a certificate
//! authority will vouch for and no address that stays the same overnight, so the usual answer —
//! *trust this because somebody you already trust says so* — has no one to be.
//!
//! What replaces it is the **fingerprint**, exchanged during pairing under the short code:
//!
//! - the machine that shows the code puts its fingerprint in what it sends;
//! - the side that dials first records the fingerprint it was shown;
//! - every connection afterwards accepts **that certificate and no other**.
//!
//! This is trust on first use, and its one weakness is stated rather than papered over: an
//! attacker already sitting between the two machines *during the five minutes of pairing* can
//! be the machine you meant to pair with. Afterwards it cannot — a swapped certificate is
//! refused, which is the property that did not exist before.
//!
//! Pinning by fingerprint is also why the certificate's *name* does not matter, and why the
//! usual hostname check is replaced rather than weakened: an address on a home network is
//! whatever DHCP said this morning, and the thing being verified here is the machine, not the
//! name it answers to.

use std::path::Path;
use std::sync::Arc;

/// A machine's certificate and the key behind it.
///
/// No `Debug`, deliberately: the key is in here, and a type that can print itself is a type that
/// eventually prints itself into a log. What a person needs to see is the fingerprint, which has
/// its own method.
pub struct Identity {
    /// The certificate, DER-encoded, as it goes on the wire.
    pub certificate: Vec<u8>,
    /// The private key, DER-encoded. Never leaves this machine.
    key: Vec<u8>,
}

impl Identity {
    /// The SHA-256 of the certificate, lower-case hex — what the two machines compare.
    ///
    /// Of the **certificate**, not of the key: it is the thing the other end actually sees.
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.certificate)
    }

    /// Serve with this identity.
    pub fn server(&self) -> Result<Arc<rustls::ServerConfig>, String> {
        let certificate = rustls_pki_types::CertificateDer::from(self.certificate.clone());
        let key = rustls_pki_types::PrivateKeyDer::try_from(self.key.clone())
            .map_err(|why| format!("this machine's key is unreadable: {why}"))?;
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], key)
            .map(Arc::new)
            .map_err(|why| format!("this machine's certificate cannot be served: {why}"))
    }
}

/// The SHA-256 of a certificate, lower-case hex.
pub fn fingerprint_of(certificate: &[u8]) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(certificate);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// This machine's identity, made once and kept.
///
/// **Made once**, because the fingerprint is what the other end pinned: generating a fresh
/// certificate every start would break every pairing on this machine at every restart, and it
/// would do it silently — the other end would simply stop being able to connect. So the pair is
/// written beside the rest of this program's state and read back afterwards.
///
/// A file that cannot be read is a **refusal, not a reason to mint a new one**. Overwriting an
/// identity because it looked odd is how a machine quietly becomes a different machine, and the
/// only honest thing to say at that point is which file is in the way.
///
/// ## An identity is two files, and it is published by one of them
///
/// It used to be written as two plain `write` calls, so a machine losing power between them kept
/// a certificate with no key — and the reader above, correctly, refuses that. Correctly and
/// permanently: the program would not start again until somebody found the file and deleted it,
/// and deleting it means re-pairing every machine.
///
/// No filesystem can rename two files at once, so this does not pretend to. **The certificate is
/// the commit.** The key is renamed into place first and the certificate second, and the reader
/// asks only whether the *certificate* is there. Every interruption therefore lands on a state
/// that means something:
///
/// | interrupted | on disk | what happens next |
/// |---|---|---|
/// | before either rename | nothing, or `.new` leftovers | a fresh identity is made |
/// | between the two renames | a key, no certificate | a fresh identity is made, over that key |
/// | after both | both | it is read, as before |
///
/// **Overwriting that orphaned key is safe and is the one case worth being sure about.** A key
/// whose certificate never landed was never served and never pinned by anybody, so nothing
/// downstream can be describing it. The rule this file exists for is untouched: a *published*
/// identity — one with a certificate — is never regenerated, only refused.
///
/// Whether the two actually match is not checked here. [`Identity::server`] builds a
/// `ServerConfig` from them at every start and says so if they do not, and a second opinion
/// about the same bytes is a second thing to keep in agreement.
pub fn identity(folder: &Path) -> Result<Identity, String> {
    let cert_at = folder.join("machine.crt");
    let key_at = folder.join("machine.key");

    if cert_at.exists() {
        let certificate = std::fs::read(&cert_at)
            .map_err(|why| format!("{} cannot be read: {why}", cert_at.display()))?;
        let key = std::fs::read(&key_at)
            .map_err(|why| format!("{} cannot be read: {why}", key_at.display()))?;
        if certificate.is_empty() || key.is_empty() {
            return Err(format!(
                "{} is empty. Delete it and this machine will make a new identity — every \
                 machine paired with this one will have to be paired again.",
                cert_at.display()
            ));
        }
        return Ok(Identity { certificate, key });
    }

    let made = new_identity()?;
    std::fs::create_dir_all(folder)
        .map_err(|why| format!("{} cannot be made: {why}", folder.display()))?;
    write_key(&key_at, &made.key)?;
    publish(&cert_at, &made.certificate, false)?;
    Ok(made)
}

/// Write one file so that it is either wholly there or not there at all.
///
/// A temporary beside it, flushed to the disk rather than only to the operating system, and then
/// a rename — which every filesystem this runs on makes atomic. Without the flush the rename can
/// land before the bytes do and a machine that loses power keeps a correctly named empty file,
/// which is the failure this is here to prevent wearing a different name.
///
/// `private` carries the Unix mode through: a key that exists world-readable for a moment has
/// been world-readable, and a rename keeps the mode the temporary was created with.
fn publish(at: &Path, bytes: &[u8], private: bool) -> Result<(), String> {
    use std::io::Write as _;

    // Appended, never `with_extension`: that one *replaces* the extension, so `machine.key` and
    // `machine.crt` would both stage through `machine.new` and the two halves of one identity
    // would be written through the same path.
    let mut staged = at.file_name().unwrap_or_default().to_os_string();
    staged.push(".new");
    let scratch = at.with_file_name(staged);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;

    let named = |why: std::io::Error| format!("{} cannot be written: {why}", at.display());
    let mut file = options.open(&scratch).map_err(named)?;
    file.write_all(bytes).map_err(named)?;
    file.sync_all().map_err(named)?;
    drop(file);
    std::fs::rename(&scratch, at).map_err(named)
}

/// Write the private key, readable by this account and nobody else where that can be said.
///
/// On Unix the mode is set **as the file is created** rather than afterwards: a key that exists
/// world-readable for a moment has been world-readable. Windows has no mode, and the folder this
/// lives in is already inside the user's own profile.
fn write_key(at: &Path, key: &[u8]) -> Result<(), String> {
    publish(at, key, true)
}

/// A fresh self-signed certificate for this machine.
///
/// The names on it are the ones a machine on a home network can honestly claim, and they are
/// **not** what is verified — see the module header. They exist because a certificate has to
/// carry something, and because a person reading it with another tool should see what it is.
fn new_identity() -> Result<Identity, String> {
    let mut names = vec!["epoch-machine".to_owned(), "localhost".to_owned()];
    if let Ok(host) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")) {
        if !host.trim().is_empty() {
            names.push(host.trim().to_ascii_lowercase());
        }
    }
    let made = rcgen::generate_simple_self_signed(names)
        .map_err(|why| format!("this machine could not make a certificate: {why}"))?;
    Ok(Identity {
        certificate: made.cert.der().to_vec(),
        key: made.signing_key.serialize_der(),
    })
}

/// Accept exactly one certificate, by fingerprint.
///
/// Everything else a verifier normally decides — the chain, the name, the dates — is answered by
/// *this is the certificate the other machine showed me when we paired*. A certificate that is
/// not that one is refused whatever else is true of it, which is stricter than the usual check
/// rather than looser: a real authority would happily vouch for a name an attacker also owns.
/// What a pinning refusal says, in one place.
///
/// **Two ends of one sentence.** `epoch-wire` writes it into a `rustls::Error::General`, and
/// `epoch-engine` reads it back out of a transport error to tell this failure apart from a
/// machine that is switched off — `ureq` reports both as `ConnectionFailed`, measured.
///
/// A constant rather than the same words typed twice, because when one fact is written down in
/// two places the agreement is a coincidence with a shelf life.
pub const NOT_THE_SAME_MACHINE: &str =
    "that machine's certificate is not the one this pairing was made with";

#[derive(Debug)]
struct Pinned {
    expected: String,
}

impl rustls::client::danger::ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &rustls_pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let seen = fingerprint_of(end_entity);
        // Constant time, for the same reason every other comparison in this codebase is: a
        // check that stops at the first wrong byte says how much was right.
        let (a, b) = (seen.as_bytes(), self.expected.as_bytes());
        let same =
            a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0;
        if same {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(NOT_THE_SAME_MACHINE.to_owned()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Record the certificate the other end shows, and accept it.
///
/// **This is the five minutes of pairing and nothing else.** It is what the far machine's
/// fingerprint is learned *by* in the direction where the code is shown on the other screen —
/// there is nothing to compare against yet, and refusing would mean two machines that can never
/// meet. Afterwards the recorded fingerprint is what [`pinned_to`] enforces, so this shape must
/// never be reachable from an ordinary turn.
#[derive(Debug)]
struct Noting {
    seen: std::sync::Mutex<Option<String>>,
}

impl rustls::client::danger::ServerCertVerifier for Noting {
    fn verify_server_cert(
        &self,
        end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &rustls_pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        if let Ok(mut held) = self.seen.lock() {
            *held = Some(fingerprint_of(end_entity));
        }
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    // `ring`, named rather than taken from the process-wide default: `ureq` installs one and this
    // crate is linked by two programs. A default somebody else set is a fact about the process,
    // and a fact about the process is not a thing to build a verifier out of.
    Arc::new(rustls::crypto::ring::default_provider())
}

fn config_with(
    verifier: Arc<dyn rustls::client::danger::ServerCertVerifier>,
) -> Arc<rustls::ClientConfig> {
    let config = rustls::ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Arc::new(config)
}

/// A client that will speak to exactly one machine: the one with this fingerprint.
pub fn pinned_to(fingerprint: &str) -> Arc<rustls::ClientConfig> {
    config_with(Arc::new(Pinned {
        expected: fingerprint.trim().to_ascii_lowercase(),
    }))
}

/// A client for the one exchange where the far machine is not known yet, and what it saw.
///
/// The caller **must** keep the fingerprint that comes back. A pairing that discards it is a
/// pairing that will trust-on-first-use again on every turn, which is the same as trusting
/// anything.
pub fn noting() -> (Arc<rustls::ClientConfig>, Arc<Learned>) {
    let learned = Arc::new(Learned {
        inner: Arc::new(Noting {
            seen: std::sync::Mutex::new(None),
        }),
    });
    let verifier: Arc<dyn rustls::client::danger::ServerCertVerifier> = learned.inner.clone();
    (config_with(verifier), learned)
}

/// What a [`noting`] client saw.
pub struct Learned {
    inner: Arc<Noting>,
}

impl Learned {
    /// The fingerprint of the certificate the far machine showed, once it has shown one.
    pub fn fingerprint(&self) -> Option<String> {
        self.inner.seen.lock().ok().and_then(|held| held.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_keeps_the_identity_it_made() {
        let folder = std::env::temp_dir().join(format!("epoch-tls-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("a temporary folder");

        let first = identity(&folder).expect("an identity");
        let again = identity(&folder).expect("the same identity");
        // **The fingerprint is what the other end pinned.** A new certificate on every start
        // would break every pairing this machine has, silently.
        assert_eq!(first.fingerprint(), again.fingerprint());
        assert_eq!(first.fingerprint().len(), 64, "sha-256 as hex");
        assert!(first.server().is_ok(), "and it can be served");

        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn two_machines_are_two_fingerprints() {
        let one = new_identity().expect("an identity");
        let two = new_identity().expect("another");
        assert_ne!(one.fingerprint(), two.fingerprint());
    }

    /// **A machine interrupted between the two files starts again, rather than never.**
    ///
    /// The state this reproduces is the one the audit named: power lost after the key landed and
    /// before the certificate did. It used to be unreachable-by-design and permanent in practice
    /// — the reader refused an identity nobody could serve, and the only way out was to find the
    /// file and delete it, which means re-pairing every machine.
    ///
    /// Overwriting that orphan is the deliberate part. A key whose certificate never landed was
    /// never served and never pinned, so nothing anywhere is describing it.
    #[test]
    fn a_key_whose_certificate_never_landed_is_not_a_dead_end() {
        let folder = std::env::temp_dir().join(format!("epoch-tls-half-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("a temporary folder");
        std::fs::write(folder.join("machine.key"), b"whatever was half-written")
            .expect("an orphaned key");

        let made = identity(&folder).expect("a machine that can start");
        assert!(folder.join("machine.crt").exists(), "and it published one");
        assert!(made.server().is_ok(), "which can actually be served");

        // And it is kept from here on, which is the rule the recovery must not have weakened.
        let again = identity(&folder).expect("the same identity");
        assert_eq!(made.fingerprint(), again.fingerprint());

        let _ = std::fs::remove_dir_all(&folder);
    }

    /// **The two halves stage through two different names.**
    ///
    /// `Path::with_extension` replaces rather than appends, so the obvious spelling would send
    /// `machine.crt` and `machine.key` through one `machine.new`. They survive it here only
    /// because they are written one after the other, which is a coincidence rather than a design
    /// and would stop being true the day anything writes them at once.
    #[test]
    fn each_half_is_staged_under_its_own_name() {
        let folder = std::env::temp_dir().join(format!("epoch-tls-stage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("a temporary folder");

        publish(&folder.join("machine.crt"), b"a certificate", false).expect("published");
        publish(&folder.join("machine.key"), b"a key", true).expect("published");

        assert_eq!(
            std::fs::read(folder.join("machine.crt")).expect("a certificate"),
            b"a certificate"
        );
        assert_eq!(
            std::fs::read(folder.join("machine.key")).expect("a key"),
            b"a key"
        );
        // Nothing staged is left behind, and nothing was staged under a shared name.
        assert!(!folder.join("machine.new").exists());
        assert!(!folder.join("machine.crt.new").exists());
        assert!(!folder.join("machine.key.new").exists());

        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn an_unreadable_identity_is_refused_rather_than_replaced() {
        // Minting a fresh one here would quietly make this a different machine, and every
        // pairing would fail with nothing saying why.
        let folder = std::env::temp_dir().join(format!("epoch-tls-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("a temporary folder");
        std::fs::write(folder.join("machine.crt"), b"").expect("an empty file");
        std::fs::write(folder.join("machine.key"), b"").expect("an empty file");

        let refused = match identity(&folder) {
            Ok(_) => panic!("an empty identity must not be replaced silently"),
            Err(why) => why,
        };
        assert!(refused.contains("machine.crt"), "{refused}");
        let _ = std::fs::remove_dir_all(&folder);
    }
}

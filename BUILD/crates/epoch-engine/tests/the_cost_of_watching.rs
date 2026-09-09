//! What it costs to notice that the vault changed.
//!
//! ## Why measure before replacing it
//!
//! The heartbeat asks `changed_on_disk()` twice a second, forever, and the roadmap's plan was to
//! replace that sweep with a filesystem watcher. A watcher is a dependency, a background thread
//! and a class of platform-specific failure — so the number comes first. If the sweep is
//! microseconds, the honest answer is that there was never a cost and the item is smaller than
//! it looked; if it is milliseconds at a realistic crew size, the watcher has earned itself.
//!
//! **The syscalls are not the interesting part.** `reload_if_changed` holds the World's lock
//! across both sweeps, so whatever this costs is time every IPC command and every projection
//! spends waiting — twice a second, for as long as the app is open. That is what the number is
//! about.
//!
//! ```text
//! cargo test -p epoch-engine --test the_cost_of_watching -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

use epoch_engine::DefinitionRegistry;

/// Crew sizes worth asking about: one person, a real crew, and a hoarder's vault.
const SIZES: [usize; 4] = [1, 5, 50, 500];

/// How many times each size is measured. The filesystem cache makes the first read the slowest,
/// and the heartbeat's question is what the *steady* cost is.
const ROUNDS: usize = 200;

fn vault(with: usize) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("epoch-watch-{with}-{n}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("characters")).expect("a vault");
    for i in 0..with {
        write_one(&dir, i);
    }
    dir
}

fn write_one(dir: &Path, i: usize) {
    std::fs::write(
        dir.join("characters").join(format!("hand{i:04}.toml")),
        format!(
            r#"
id = "hand{i:04}"
name = "Hand {i}"
archetype = "researcher"
role = "One of many"
prompt = "You do your part."
worlds = ["default"]

[presence]
home = "research_lab"
idle = [
  {{ activity = "reading", seconds = 12 }},
]
"#
        ),
    )
    .expect("a character");
}

#[test]
#[ignore = "a measurement of this machine's filesystem, at four crew sizes"]
fn noticing_a_changed_vault_costs_this_much() {
    println!("crew   per check   per second (2 checks)");
    for size in SIZES {
        let dir = vault(size);
        let registry = DefinitionRegistry::load(&dir);
        assert_eq!(
            registry.loaded().count(),
            size,
            "the vault really has {size}"
        );

        // Warm, then measure: the first sweep pays for a cold directory cache and the heartbeat
        // never sees that price again.
        let _ = registry.changed_on_disk();

        let began = Instant::now();
        for _ in 0..ROUNDS {
            assert!(
                !registry.changed_on_disk(),
                "nothing changed, so this must say so"
            );
        }
        let each = began.elapsed() / ROUNDS as u32;

        println!(
            "{size:<6} {:>9?}   {:>6.3} ms/s",
            each,
            each.as_secs_f64() * 2.0 * 1000.0
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_changed_file_is_actually_noticed() {
    // The measurement above is only meaningful if the thing being measured works: a sweep that
    // answered `false` quickly would be fast and useless.
    let dir = vault(5);
    let registry = DefinitionRegistry::load(&dir);
    assert!(!registry.changed_on_disk());

    // A new file is a change even though every file it already knew is untouched.
    write_one(&dir, 99);
    assert!(registry.changed_on_disk(), "a new character is a change");

    let _ = std::fs::remove_dir_all(&dir);
}

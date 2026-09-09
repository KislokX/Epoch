//! What it costs to notice that a World's artwork changed.
//!
//! ## Why measure before building it
//!
//! Characters hot-reload: the heartbeat asks `changed_on_disk()` twice a second and a definition
//! edited in another window reaches the next turn. A World Pack's artwork does not — `pack.rs`
//! keeps the resolved `data:` URI and never the path it came from, so editing a sprite means
//! leaving the World and entering it again.
//!
//! Closing that needs the pack to answer the same question, and the cheap way is to walk its
//! folder. Whether that is cheap depends on a number nobody has: `reload_if_changed` holds the
//! World's lock across the sweep, so whatever this costs is time every IPC command and every
//! projection spends waiting, twice a second, for as long as the app is open.
//!
//! The packs on this machine have two and four files. A World somebody actually drew has
//! hundreds, and that is the size the answer has to hold at.
//!
//! ```text
//! cargo test -p epoch-engine --test the_cost_of_watching_a_pack -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

/// Pack sizes worth asking about: the ones that exist, one somebody drew, and a hoarder's.
const SIZES: [usize; 4] = [4, 50, 500, 2000];

/// The filesystem cache makes the first sweep the slowest, and the heartbeat's question is what
/// the *steady* cost is.
const ROUNDS: usize = 200;

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pack_of(files: usize) -> Dir {
    let dir = std::env::temp_dir().join(format!("epoch-pack-cost-{files}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // Nested, because artwork is: sprites under places under the pack.
    for n in 0..files {
        let at = dir.join(format!("art/{}", n % 8));
        std::fs::create_dir_all(&at).unwrap();
        std::fs::write(at.join(format!("{n}.png")), b"not really a png").unwrap();
    }
    Dir(dir)
}

/// The shape of a sweep: every file under the pack, folded into one number.
///
/// **The same shape as the real one, twice over.** The first version took the newest mtime —
/// cheaper, and blind to a file being *restored*, which is what an undo is. The second sorted the
/// files to make the fold order-independent, which cost 5x. This one adds per-file hashes, which
/// is order-independent without the sort. A measurement of a cheaper thing than the one that
/// ships is a measurement of nothing.
fn sweep(dir: &Path) -> u64 {
    let mut total: u64 = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let when = meta
                .modified()
                .ok()
                .and_then(|w| w.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0u64, |d| d.as_nanos() as u64);
            let mut one: u64 = 0xcbf2_9ce4_8422_2325;
            for byte in entry
                .file_name()
                .to_string_lossy()
                .as_bytes()
                .iter()
                .chain(meta.len().to_le_bytes().iter())
                .chain(when.to_le_bytes().iter())
            {
                one ^= u64::from(*byte);
                one = one.wrapping_mul(0x1000_0000_01b3);
            }
            total = total.wrapping_add(one);
        }
    }
    total
}

#[test]
#[ignore = "a measurement, not an assertion"]
fn what_a_sweep_of_a_pack_costs() {
    println!(
        "{:>7}  {:>10}  {:>12}  {:>14}",
        "files", "per sweep", "twice a sec", "of wall time"
    );
    for files in SIZES {
        let pack = pack_of(files);
        // Warm the cache: the steady cost is the question, not the first read.
        for _ in 0..5 {
            sweep(&pack.0);
        }
        let started = Instant::now();
        for _ in 0..ROUNDS {
            std::hint::black_box(sweep(&pack.0));
        }
        let each = started.elapsed().as_secs_f64() * 1000.0 / ROUNDS as f64;
        println!(
            "{files:>7}  {:>9.2}ms  {:>11.2}ms  {:>13.2}%",
            each,
            each * 2.0,
            each * 2.0 / 1000.0 * 100.0
        );
    }
}

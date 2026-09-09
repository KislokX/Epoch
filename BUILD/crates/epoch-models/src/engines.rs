//! Which GPU backend a runtime uses, and whether this machine can be told to change it.
//!
//! ## Why this exists at all, having been refused once
//!
//! A per-card **CUDA · Vulkan · Auto** chooser was proposed and killed by measurement (11.20):
//! one llama.cpp build carries one GPU backend, so there was nothing to switch, and a menu
//! assembled from what is *conceivable* will eventually offer somebody `Intel Arc: CUDA` — a
//! configuration that cannot exist.
//!
//! That answer was right about llama.cpp and wrong as a general claim. Measured again on
//! 2026-08-30:
//!
//! - **Ollama ships four GPU backends in one install** — `cuda_v12`, `cuda_v13`, `rocm_v7_1`,
//!   `vulkan` — and `OLLAMA_LLM_LIBRARY` picks between them. Verified by using it rather than by
//!   reading the help: started with `vulkan`, its log says `library=Vulkan` and
//!   `using device Vulkan`, and the model answered on an NVIDIA card through Vulkan on purpose.
//! - **LM Studio ships several engines** and `lms runtime select` picks one.
//! - **llama.cpp still carries exactly one**, which is why its answer here is a sentence rather
//!   than a menu.
//!
//! **The rule that killed the first proposal is what makes this one buildable.** Nothing below is
//! a list of backends that exist in the world; every row is read from what is installed on this
//! machine — Ollama's own backend directory, and `lms runtime ls`. A machine with one engine gets
//! one row, and nobody is ever offered a combination that does not exist.
//!
//! **And a name is never invented.** Where a directory or an engine id cannot be recognised it is
//! shown as it is written. A pretty label for something Epoch could not identify would be a guess
//! wearing the clothes of a measurement.

use serde::{Deserialize, Serialize};

use crate::runtimes::{found_in, Runtime};

/// One GPU backend that is installed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    /// What the program is told: a directory name for Ollama, an engine key for LM Studio.
    pub id: String,
    /// What a person reads. The id itself when Epoch cannot recognise it.
    pub name: String,
    /// Whether the program says this is the one in use. Only LM Studio reports it.
    pub chosen: bool,
}

/// What this machine can be told about a runtime's backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "of")]
pub enum Engines {
    /// Several are installed and one can be picked.
    Choice(Vec<Engine>),
    /// Exactly one, named. Changing it means installing a different build.
    Fixed(String),
    /// Nothing could be read — the program is not here, or its layout is not one Epoch knows.
    ///
    /// **Never "there is only CPU".** No reading beats an invented one, and this is the case
    /// where a surface must say *nobody asked* rather than answer for the machine.
    Unknown,
}

/// What a runtime's backends are on this machine.
pub fn engines(runtime: Runtime) -> Engines {
    match runtime {
        Runtime::Ollama => ollama(),
        Runtime::LmStudio => lm_studio(),
        Runtime::LlamaCpp => llama_cpp(),
    }
}

/// Ollama keeps its backends in directories beside the server it spawns.
///
/// Read from disk rather than from a list of names Ollama might ship, so a build with three
/// backends offers three and a build with one offers one.
fn ollama() -> Engines {
    let Some(at) = found_in(
        "ollama",
        Runtime::Ollama.winget_package(),
        Runtime::Ollama.program_folder(),
    ) else {
        return Engines::Unknown;
    };
    let Some(home) = beside(&at) else {
        return Engines::Unknown;
    };
    let home = home.as_path();
    /*
        **Three layouts, measured rather than assumed.**

        Windows: `lib/ollama/<backend>` beside `ollama.exe`. A `bin`-style install puts the
        binary one level down from `lib`. And on macOS the whole application is one directory —
        `Ollama.app/Contents/Resources/` holds the binary *and* `mlx_metal_v3` / `mlx_metal_v4`
        beside it, with no `lib` anywhere. Read from the owner's own MacBook; a function that
        only knew the first two answered `Unknown` on the machine where the question — *is this
        using the GPU?* — is hardest to answer by eye.
    */
    let places = [
        home.join("lib").join("ollama"),
        home.join("..").join("lib").join("ollama"),
        home.to_path_buf(),
    ];
    let Some(lib) = places.into_iter().find(|it| it.is_dir()) else {
        return Engines::Unknown;
    };
    let Ok(reading) = std::fs::read_dir(&lib) else {
        return Engines::Unknown;
    };

    let mut found: Vec<Engine> = reading
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter(|entry| holds_a_backend(&entry.path()))
        .map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            Engine {
                name: readable(&id),
                id,
                chosen: false,
            }
        })
        .collect();
    if found.is_empty() {
        return Engines::Unknown;
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Engines::Choice(found)
}

/// The directory a program's libraries actually sit in, following any launcher symlink first.
///
/// **A symlink says nothing about where the libraries are.** Measured on the owner's MacBook:
/// `/usr/local/bin/ollama` points into `Ollama.app/Contents/Resources/`, where the backends live
/// — and reading the *link's* parent finds `/usr/local/bin`, which holds nothing, so a machine
/// with two Metal backends installed answered `Unknown`. Following it is one call and it is the
/// general fix: package managers and application bundles both install this way.
fn beside(at: &std::path::Path) -> Option<std::path::PathBuf> {
    let real = std::fs::canonicalize(at).unwrap_or_else(|_| at.to_path_buf());
    real.parent().map(std::path::Path::to_path_buf)
}

/// A directory is a backend if it holds one — never because of what it is called.
///
/// **What a backend *is*, rather than a list of names.** This asked for a file beginning `ggml-`,
/// which is true of Ollama's CUDA and Vulkan directories on Windows and of nothing at all on a
/// Mac: `mlx_metal_v3` holds `libmlx.dylib`, because Apple's path through Ollama is MLX and not
/// ggml. A name list would have needed a new entry the day a fourth backend shipped; a loadable
/// library is what every one of them has in common.
fn holds_a_backend(at: &std::path::Path) -> bool {
    std::fs::read_dir(at).is_ok_and(|reading| {
        reading.flatten().any(|entry| {
            let file = entry.file_name().to_string_lossy().to_ascii_lowercase();
            file.ends_with(".dll") || file.ends_with(".dylib") || file.ends_with(".so")
        })
    })
}

/// LM Studio publishes its engines and says which one is selected — **when it is already up.**
///
/// ## A reading probe may not start the program it is reading
///
/// This ran `lms runtime ls` unconditionally, and that CLI starts LM Studio. So opening
/// CONNECTIONS — which mounts a panel whose only job is to say what exists here — launched a
/// program that then held gigabytes of memory. Reported by the owner three times before anybody
/// looked at the right function; the comment above the caller says *"this is a directory listing
/// and one CLI call"*, and that one call was the whole defect.
///
/// **Asking has a cost that a person did not agree to pay.** It is the same rule that keeps a
/// picture server stopped until somebody presses GENERATE, arriving in a probe instead of a
/// capability.
///
/// So: ask the program only where the program is already answering. Otherwise read the same fact
/// off the disk, exactly as Ollama's is read — the backends are directories, and a directory
/// listing starts nothing.
///
/// **What the disk cannot say is which one is selected**, and that is left unsaid rather than
/// guessed. A passive reading answers *these are installed*; it does not answer *this is the one
/// in use*, and inventing a mark on one of them would be a gauge nobody measured.
fn lm_studio() -> Engines {
    if crate::runtimes::look_for(Runtime::LmStudio).serving {
        if let Some(lms) = found_in("lms", None, None) {
            if let Ok(said) = std::process::Command::new(lms)
                .args(["runtime", "ls"])
                .output()
            {
                let text = String::from_utf8_lossy(&said.stdout);
                let found: Vec<Engine> = text.lines().filter_map(read_engine_row).collect();
                if !found.is_empty() {
                    return Engines::Choice(found);
                }
            }
        }
    }
    lm_studio_on_disk()
}

/// LM Studio's installed backends, read from where it keeps them.
///
/// `~/.lmstudio/extensions/backends/<engine>`, and every entry is a directory holding a loadable
/// library — the same test Ollama's uses, because *what a backend is* survives a layout change
/// and a list of names does not.
fn lm_studio_on_disk() -> Engines {
    let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
    else {
        return Engines::Unknown;
    };
    let backends = home.join(".lmstudio").join("extensions").join("backends");
    let Ok(reading) = std::fs::read_dir(&backends) else {
        return Engines::Unknown;
    };
    let mut found: Vec<Engine> = reading
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter(|entry| holds_a_backend(&entry.path()))
        .map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            Engine {
                name: readable(&id),
                id,
                // **Nobody was asked.** Only the running program knows which engine it selected.
                chosen: false,
            }
        })
        .collect();
    if found.is_empty() {
        return Engines::Unknown;
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Engines::Choice(found)
}

/// One row of `lms runtime ls`, or nothing for a header or a blank.
///
/// The selected engine is marked with a tick in its own column, so this reads the mark rather
/// than the position — a column that moves would otherwise silently mark the wrong row.
fn read_engine_row(line: &str) -> Option<Engine> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("LLM ENGINE") {
        return None;
    }
    let id = trimmed.split_whitespace().next()?;
    // Every engine LM Studio lists carries its own version after an `@`; a line without one is
    // not a row of this table.
    if !id.contains('@') {
        return None;
    }
    /*
        **The version is part of the name here, and leaving it off made three rows identical.**

        Read from this machine: `nvidia-cuda12-avx2@2.31.2`, `@2.29.1` and `@2.28.2` are three
        installed engines that all resolve to `CUDA 12`. A menu of three rows reading the same
        thing is a gauge that identifies nobody — correct readings of the right quantity, with
        nothing saying which is which.

        It is a qualifier rather than part of the family, because `2.31.2` is LM Studio's
        packaging of llama.cpp and not CUDA's version.
    */
    let (family, version) = id.split_once('@').unwrap_or((id, ""));
    let named = readable(id);
    let head = if named == id {
        family.to_owned()
    } else {
        named
    };
    Some(Engine {
        name: if version.is_empty() {
            head
        } else {
            format!("{head} \u{00b7} {version}")
        },
        id: id.to_owned(),
        chosen: trimmed.contains('\u{2713}'),
    })
}

/// llama.cpp carries one backend per install, so the honest answer is which one.
fn llama_cpp() -> Engines {
    let package = Runtime::LlamaCpp.winget_package();
    let Some(at) =
        found_in("llama-server", package, None).or_else(|| found_in("llama", package, None))
    else {
        return Engines::Unknown;
    };
    let Some(home) = beside(&at) else {
        return Engines::Unknown;
    };
    let Ok(reading) = std::fs::read_dir(&home) else {
        return Engines::Unknown;
    };

    /*
        **Only a backend that is a card.**

        Run against this machine's own install, excluding `base` and `cpu` was not enough: the
        first reading came back `CUDA - rpc - rpc-server`, because `ggml-rpc` is a *transport*
        that forwards work to another host and ships beside the GPU backend in every build.

        So the test is the one already used to name things — a file counts only where `readable`
        recognises a family in it. A name Epoch cannot place is not claimed to be the card, which
        is the same discipline as leaving it verbatim in a menu: silence beats a wrong answer to
        the question actually being asked, and the question here is *which GPU backend is this*.
    */
    let mut names: Vec<String> = reading
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let rest = file.strip_prefix("ggml-")?;
            let (stem, _) = rest.split_once('.')?;
            let name = readable(stem);
            (name != stem).then_some(name)
        })
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Engines::Unknown;
    }
    Engines::Fixed(names.join(" · "))
}

/// A name a person reads, or the id itself when Epoch does not recognise it.
///
/// Recognition is by what the id *contains*, because the three programs spell the same backend
/// three ways — `cuda_v12`, `nvidia-cuda12`, `ggml-cuda`. What is not recognised is shown
/// verbatim rather than guessed at.
fn readable(id: &str) -> String {
    let low = id.to_ascii_lowercase();
    let family = if low.contains("cuda") {
        "CUDA"
    } else if low.contains("rocm") || low.contains("hip") {
        "ROCm"
    } else if low.contains("vulkan") {
        "Vulkan"
    } else if low.contains("sycl") {
        "SYCL"
    } else if low.contains("metal") || low.contains("mlx") {
        // Ollama's Apple path is `mlx_metal_v3`: MLX is the framework, Metal is the card.
        "Metal"
    } else {
        return id.to_owned();
    };
    match version_in(&low) {
        Some(version) => format!("{family} {version}"),
        None => family.to_owned(),
    }
}

/// The version written after a backend's name: `cuda_v12` is 12, `rocm_v7_1` is 7.1.
///
/// Only digits that immediately follow the family are read. A trailing `@2.31.2` on an LM Studio
/// engine is that *engine's* version rather than the backend's, and calling it `CUDA 2.31.2`
/// would be a real number attached to the wrong quantity.
fn version_in(low: &str) -> Option<String> {
    /*
        **A directory says its version at the end**, and that is where two of them differ.

        `mlx_metal_v3` and `mlx_metal_v4` are both installed on the owner's MacBook, and reading
        the version only where it follows a family name left them both called `Metal` — the same
        three-identical-rows defect the LM Studio list had, on the machine with the fewest rows.

        Tried first because it is the more specific shape: `cuda_v12` and `rocm_v7_1` carry the
        same suffix and answer identically either way.
    */
    if let Some((_, tail)) = low.rsplit_once("_v") {
        let digits: String = tail
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '_' || *c == '.')
            .collect();
        let cleaned = digits.trim_end_matches(['_', '.']).replace('_', ".");
        if !cleaned.is_empty() && digits.len() == tail.len() {
            return Some(cleaned);
        }
    }
    let after = low
        .split_once("cuda")
        .or_else(|| low.split_once("rocm"))
        .or_else(|| low.split_once("hip"))
        .map(|(_, rest)| rest)?;
    let digits: String = after
        .trim_start_matches(['_', '-', 'v'])
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '_' || *c == '.')
        .collect();
    let cleaned = digits.trim_end_matches(['_', '.']).replace('_', ".");
    if cleaned.is_empty() {
        return None;
    }
    Some(cleaned)
}

/// Tell LM Studio which engine to use. Ollama is told when it starts instead.
pub fn choose(id: &str) -> Result<String, String> {
    let lms = found_in("lms", None, None).ok_or("LM Studio's CLI is not on this machine")?;
    let said = std::process::Command::new(lms)
        .args(["runtime", "select", id])
        .output()
        .map_err(|why| format!("LM Studio's CLI could not be run ({why})"))?;
    if !said.status.success() {
        // The program's own words: it knows why far better than a sentence written here.
        let complaint = String::from_utf8_lossy(&said.stderr).trim().to_owned();
        return Err(if complaint.is_empty() {
            format!("LM Studio would not select {id}.")
        } else {
            complaint
        });
    }
    Ok(format!("LM Studio will use {}.", readable(id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lm_studios_backends_are_readable_without_starting_it() {
        /*
            The directory names on this machine, 2026-09-01. Read from
            `~/.lmstudio/extensions/backends`, which is where LM Studio keeps them — and reading a
            directory starts nothing, which is the whole point: `lms runtime ls` launches the
            program, and a panel that only wants to say what exists here was doing that on mount.
        */
        for (dir, family) in [
            ("llama.cpp-win-x86_64-nvidia-cuda12-avx2-2.31.2", "CUDA"),
            ("llama.cpp-win-x86_64-vulkan-avx2-2.28.2", "Vulkan"),
        ] {
            assert!(
                readable(dir).contains(family),
                "{dir} should read as {family}, got {}",
                readable(dir),
            );
        }
        // A CPU build names no family, and is shown as it is written rather than guessed at.
        assert_eq!(
            readable("llama.cpp-win-x86_64-avx2-2.28.2"),
            "llama.cpp-win-x86_64-avx2-2.28.2",
        );
    }

    #[test]
    fn a_backend_is_named_by_what_it_is_not_by_who_ships_it() {
        // The three programs spell one backend three ways.
        assert_eq!(readable("cuda_v12"), "CUDA 12");
        assert_eq!(readable("cuda_v13"), "CUDA 13");
        assert_eq!(readable("rocm_v7_1"), "ROCm 7.1");
        assert_eq!(readable("vulkan"), "Vulkan");
        assert_eq!(
            readable("llama.cpp-win-x86_64-vulkan-avx2@2.28.2"),
            "Vulkan"
        );
        assert_eq!(readable("cuda"), "CUDA");
    }

    #[test]
    fn an_engines_version_is_not_the_backends_version() {
        // `@2.31.2` belongs to LM Studio's packaging of llama.cpp, not to CUDA. A real number
        // attached to the wrong quantity is the most convincing way an instrument can lie.
        assert_eq!(
            readable("llama.cpp-win-x86_64-nvidia-cuda12-avx2@2.31.2"),
            "CUDA 12"
        );
    }

    #[test]
    fn apples_two_backends_stay_two_things() {
        // Measured on the owner's MacBook: `Ollama.app/Contents/Resources` holds both of these
        // beside the binary, and calling them both `Metal` is a menu that cannot be used.
        assert_eq!(readable("mlx_metal_v3"), "Metal 3");
        assert_eq!(readable("mlx_metal_v4"), "Metal 4");
        assert_ne!(readable("mlx_metal_v3"), readable("mlx_metal_v4"));
    }

    #[test]
    fn what_epoch_cannot_recognise_is_shown_as_it_is_written() {
        // A pretty label for an unidentified thing is a guess dressed as a measurement.
        assert_eq!(readable("some_new_backend_v9"), "some_new_backend_v9");
        assert_eq!(readable("avx2"), "avx2");
    }

    #[test]
    fn the_header_and_the_blank_lines_are_not_engines() {
        assert!(read_engine_row("LLM ENGINE      SELECTED    MODEL FORMAT").is_none());
        assert!(read_engine_row("   ").is_none());
        assert!(read_engine_row("nothing here").is_none());
    }

    #[test]
    fn the_selected_engine_is_read_from_its_mark() {
        // The measured shape, from this machine's own `lms runtime ls`.
        let row = read_engine_row(
            "llama.cpp-win-x86_64-nvidia-cuda12-avx2@2.31.2       \u{2713}            GGUF",
        )
        .expect("that is a row");
        assert_eq!(row.id, "llama.cpp-win-x86_64-nvidia-cuda12-avx2@2.31.2");
        assert_eq!(row.name, "CUDA 12 \u{00b7} 2.31.2");
        assert!(row.chosen);

        let other =
            read_engine_row("llama.cpp-win-x86_64-vulkan-avx2@2.28.2                    GGUF")
                .expect("that is a row");
        assert_eq!(other.name, "Vulkan \u{00b7} 2.28.2");
        assert!(!other.chosen);
    }

    #[test]
    fn three_engines_of_one_family_stay_three_things() {
        // Measured on this machine: three CUDA 12 engines are installed, and a menu that calls
        // them all `CUDA 12` cannot be used to pick one.
        let names: Vec<String> = [
            "llama.cpp-win-x86_64-nvidia-cuda12-avx2@2.31.2   GGUF",
            "llama.cpp-win-x86_64-nvidia-cuda12-avx2@2.29.1   GGUF",
            "llama.cpp-win-x86_64-nvidia-cuda12-avx2@2.28.2   GGUF",
        ]
        .iter()
        .filter_map(|line| read_engine_row(line))
        .map(|it| it.name)
        .collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "{names:?}");

        // And an engine with no family Epoch recognises keeps its own name, minus the version
        // that is now a qualifier.
        let cpu = read_engine_row("llama.cpp-win-x86_64-avx2@2.28.2   GGUF").expect("a row");
        assert_eq!(cpu.name, "llama.cpp-win-x86_64-avx2 \u{00b7} 2.28.2");
    }
}

//! The whole path, with a real ComfyUI: a Style resolves, a workflow compiles, a picture lands.
//!
//! `#[ignore]` — it needs a server with a checkpoint, and it writes into a temporary vault.
//!
//! It exists because every other test here holds one rule still, and the four defects that
//! actually cost time were between the parts: a converter that read a workflow the server then
//! refused, a widget order that produced a plausible wrong request, an input that looked like a
//! socket. Only a run catches those.

use std::path::PathBuf;

fn nowhere() -> PathBuf {
    std::env::temp_dir().join(format!("epoch-draws-{}", std::process::id()))
}

#[test]
#[ignore = "needs ComfyUI serving on 8188 with a checkpoint"]
fn a_style_becomes_a_picture() {
    let vault = nowhere();
    let _ = std::fs::remove_dir_all(&vault);
    std::fs::create_dir_all(&vault).expect("a vault to write into");

    let comfy = epoch_engine::comfy::Comfy::at("http://127.0.0.1:8188");
    let schema = comfy.schema().expect("ComfyUI answers");

    // The workflow somebody would import: ComfyUI's own, in the format its editor saves.
    let template = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap())
        .join("Comfy-Desktop/ComfyUI-Installs/ComfyUI/ComfyUI/.venv/Lib/site-packages")
        .join("comfyui_workflow_templates_json/templates/sdxl_simple_example.json");
    let Ok(raw) = std::fs::read_to_string(&template) else {
        eprintln!("no template at {}", template.display());
        return;
    };
    let mut source: serde_json::Value = serde_json::from_str(&raw).expect("it is JSON");
    // This machine has the base and not the refiner. A substitution for the *test*, never
    // something Epoch does to somebody's workflow.
    if let Some(nodes) = source["nodes"].as_array_mut() {
        for node in nodes {
            if node["type"] == "CheckpointLoaderSimple" {
                if let Some(values) = node["widgets_values"].as_array_mut() {
                    values[0] = serde_json::json!("sd_xl_base_1.0.safetensors");
                }
            }
        }
    }

    let mut images = epoch_engine::images::Images::first_run();
    let id = images
        .take_in(&vault, "Basic SDXL", &source, &schema)
        .expect("it is taken in");
    images
        .styles
        .iter_mut()
        .find(|style| style.name == "General")
        .expect("General ships")
        .workflows
        .push(id.clone());
    images.save(&vault).expect("saved");

    // Read back from disk, which is what a real run does — the file is the source of truth and
    // the in-memory copy would hide a serialisation that did not round trip.
    let images = epoch_engine::images::Images::load(&vault);
    let kept = images.serving("General").expect("General is lit");
    assert_eq!(kept.id, id);
    let opening = kept.opening.as_ref().expect("it compiled here");
    assert!(opening.can.from_words, "{opening:?}");

    let mut workflow = images.open(&vault, &id, &schema).expect("it opens");
    workflow
        .compiled
        .say("a small red lighthouse on a white cliff", None);
    workflow.compiled.shape(1024, 1024);
    workflow.compiled.seed(7);

    let (bytes, name, seconds) = comfy.draw(&workflow).expect("it draws");
    assert!(
        bytes.len() > 10_000,
        "{} bytes is not a picture",
        bytes.len()
    );
    assert!(name.ends_with(".png"), "{name}");

    // Into the vault, under a name taken from the bytes — where the Chronicle draws from.
    let file = epoch_engine::import::keep_bytes(&vault, &bytes).expect("kept");
    let kept_at = epoch_engine::import::shared_images(&vault).join(&file);
    assert!(kept_at.is_file(), "{}", kept_at.display());
    assert_eq!(
        std::fs::read(&kept_at).expect("readable").len(),
        bytes.len(),
        "the same bytes, not a re-encode"
    );

    // The same picture twice is one file: the name is the content.
    let again = epoch_engine::import::keep_bytes(&vault, &bytes).expect("kept");
    assert_eq!(again, file);

    println!("drew {file} in {seconds:.1}s ({} bytes)", bytes.len());
    let _ = std::fs::remove_dir_all(&vault);
}

/// Handing ComfyUI a picture to work from.
///
/// Measured 2026-08-22 rather than assumed: `POST /upload/image` takes multipart and answers
/// `{"name": …, "type": "input"}`, and that name is what a `LoadImage` node loads by. Epoch's own
/// filename means nothing there, and a path from this machine means less — a lent ComfyUI has its
/// own disk.
///
/// This asserts the handover and the wiring, not a picture: an img2img run needs a workflow whose
/// checkpoint is installed, and what could go wrong *here* is the upload and the node's value.
#[test]
#[ignore = "needs ComfyUI serving on 8188"]
fn a_reference_picture_is_handed_over_and_wired_in() {
    let comfy = epoch_engine::comfy::Comfy::at("http://127.0.0.1:8188");
    let schema = comfy.schema().expect("ComfyUI answers");

    // A 1×1 PNG, written by hand: a fixture with no dependencies and nothing ambiguous in it.
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let there = comfy
        .hand_over("epoch-reference.png", PNG)
        .expect("ComfyUI takes it");
    assert_eq!(
        there, "epoch-reference.png",
        "it keeps the name it was given"
    );

    // A graph that loads a picture. Two nodes: enough to prove `show` finds the loader and the
    // server accepts what it was set to.
    let source = serde_json::json!({
        "1": { "class_type": "LoadImage", "inputs": { "image": "example.png" } },
        "2": { "class_type": "PreviewImage", "inputs": { "images": ["1", 0] } }
    });
    let mut taken = epoch_assets::workflow::Imported::of(source, &schema).expect("it reads");
    assert!(taken.opening.can.from_image, "{:?}", taken.opening);
    assert_eq!(taken.opening.takes_image, vec!["1"], "{:?}", taken.opening);

    taken.compiled.show(&there);
    assert_eq!(
        taken.compiled.nodes["1"].inputs["image"],
        epoch_assets::workflow::Input::Value(serde_json::json!("epoch-reference.png"))
    );

    // And the server takes the graph — which is the half a unit test cannot reach: a name it
    // does not recognise comes back `value_not_in_list`.
    let (bytes, _name, _seconds) = comfy.draw(&taken).expect("it runs");
    assert!(!bytes.is_empty(), "it previewed the picture it was handed");
}

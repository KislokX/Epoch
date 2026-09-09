//! What the Studio Panel composes, drawn by a real ComfyUI (ADR-0033).
//!
//! Ignored, like every other measurement that needs a server: it queues a real render, waits for
//! it, and writes the picture where a person can look at it. The unit tests hold the *shape* of
//! the graph; only this holds that the shape is one a server will actually execute with a LoRA
//! chained into it.
//!
//! Run it with:
//!
//! ```text
//! cargo test -p epoch-assets --test the_panel_draws -- --ignored --nocapture
//! ```

use std::time::Duration;

use epoch_assets::workflow::{compose, Ask, Loading, Schema};

const COMFY: &str = "http://127.0.0.1:8188";

#[test]
#[ignore = "needs ComfyUI serving on 8188"]
fn a_lora_from_the_library_reaches_the_picture() {
    let info: serde_json::Value = ureq::get(&format!("{COMFY}/object_info"))
        .timeout(Duration::from_secs(60))
        .call()
        .expect("ComfyUI answered")
        .into_json()
        .expect("readable");

    let checkpoint = info["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"][0][0]
        .as_str()
        .expect("this ComfyUI reports a checkpoint")
        .to_owned();

    // The LoRA list is the server's, exactly like the checkpoint list — which is what proves the
    // search path Epoch added is being honoured rather than merely written.
    let loras: Vec<String> = info["LoraLoader"]["input"]["required"]["lora_name"][0]
        .as_array()
        .map(|it| {
            it.iter()
                .filter_map(|name| name.as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    println!("checkpoint: {checkpoint}");
    println!("loras this server can load: {loras:?}");

    let lora = loras
        .iter()
        .find(|name| name.contains("pixel-art-xl"))
        .cloned();
    assert!(
        lora.is_some(),
        "ComfyUI does not list the LoRA Epoch installed — the search path is written and not \
         honoured, or it needs a restart: {loras:?}"
    );

    let mut ask = Ask::of(
        &checkpoint,
        "asuka langley soryu from evangelion, red plugsuit, pixel",
    );
    ask.loras = vec![(lora.unwrap(), 0.9)];
    ask.width = 1024;
    ask.height = 1024;
    ask.seed = 7;

    let schema = Schema::read(&info);
    let graph = compose(&ask, &schema).expect("it composed");

    let queued: serde_json::Value = ureq::post(&format!("{COMFY}/prompt"))
        .timeout(Duration::from_secs(60))
        .send_json(serde_json::json!({"prompt": graph}))
        .expect("ComfyUI accepted it")
        .into_json()
        .expect("readable");
    let id = queued["prompt_id"]
        .as_str()
        .expect("a prompt id")
        .to_owned();
    println!("queued {id}");

    // Polled rather than streamed: this is a test, and the server's own history is the record.
    let mut made: Option<serde_json::Value> = None;
    for _ in 0..180 {
        std::thread::sleep(Duration::from_secs(1));
        let history: serde_json::Value = ureq::get(&format!("{COMFY}/history/{id}"))
            .timeout(Duration::from_secs(30))
            .call()
            .expect("history answered")
            .into_json()
            .expect("readable");
        if let Some(entry) = history.get(&id) {
            made = Some(entry.clone());
            break;
        }
    }
    let made = made.expect("it finished within three minutes");

    let image = made["outputs"]
        .as_object()
        .expect("outputs")
        .values()
        .filter_map(|node| node.get("images")?.as_array()?.first().cloned())
        .next()
        .expect("something was saved");
    let filename = image["filename"].as_str().expect("a filename");
    let subfolder = image["subfolder"].as_str().unwrap_or("");
    let kind = image["type"].as_str().unwrap_or("output");

    let bytes = {
        let answer = ureq::get(&format!(
            "{COMFY}/view?filename={filename}&subfolder={subfolder}&type={kind}"
        ))
        .timeout(Duration::from_secs(60))
        .call()
        .expect("the picture came back");
        let mut held = Vec::new();
        std::io::Read::read_to_end(&mut answer.into_reader(), &mut held).expect("read");
        held
    };
    assert!(
        bytes.len() > 10_000,
        "that is not a picture: {} bytes",
        bytes.len()
    );

    // Somewhere a person can open it. The vault is the app's business; a test writes to temp.
    let out = std::env::temp_dir().join("epoch-panel-drew.png");
    std::fs::write(&out, &bytes).expect("written");
    println!("drew {} bytes -> {}", bytes.len(), out.display());
}

/// The Flux recipe, drawn by a real ComfyUI.
///
/// A different graph from the checkpoint one — `UNETLoader` + `DualCLIPLoader` + `VAELoader`,
/// a sixteen-channel latent, and guidance instead of cfg — so it is proved separately or not at
/// all.
#[test]
#[ignore = "needs ComfyUI serving on 8188 with Flux installed"]
fn the_flux_recipe_draws() {
    let info: serde_json::Value = ureq::get(&format!("{COMFY}/object_info"))
        .timeout(Duration::from_secs(60))
        .call()
        .expect("ComfyUI answered")
        .into_json()
        .expect("readable");

    let pick = |node: &str, input: &str, needle: &str| -> Option<String> {
        info[node]["input"]["required"][input][0]
            .as_array()?
            .iter()
            .filter_map(|name| name.as_str())
            .find(|name| name.to_ascii_lowercase().contains(needle))
            .map(str::to_owned)
    };

    let unet = pick("UNETLoader", "unet_name", "flux").expect("a Flux diffusion model");
    let t5 = pick("DualCLIPLoader", "clip_name1", "t5").expect("a T5 encoder");
    let clip_l = pick("DualCLIPLoader", "clip_name1", "clip_l").expect("clip_l");
    let vae = pick("VAELoader", "vae_name", "vae").expect("a Flux VAE");
    println!(
        "unet {unet}
clip {t5} + {clip_l}
vae {vae}"
    );

    let mut ask = Ask::of(
        "unused",
        "japanese retro aesthetic, asuka langley soryu in a red plugsuit, 1980s VHS",
    );
    ask.loading = Loading::Assembled {
        unet,
        clip: vec![t5, clip_l],
        clip_type: "flux".to_owned(),
        vae,
    };
    // The LoRA, when this machine has one for Flux. Chained exactly as it is onto a checkpoint —
    // the same wiring, which is why it lives outside the branch.
    if let Some(lora) = info["LoraLoader"]["input"]["required"]["lora_name"][0]
        .as_array()
        .and_then(|all| {
            all.iter()
                .filter_map(|name| name.as_str())
                .find(|name| name.contains("retro_aesthetic"))
        })
    {
        println!("with lora {lora}");
        ask.loras = vec![(lora.to_owned(), 0.9)];
    }
    ask.width = 1024;
    ask.height = 1024;
    ask.steps = 20;
    ask.seed = 3;

    let graph = compose(&ask, &Schema::read(&info)).expect("it composed");
    let queued: serde_json::Value = ureq::post(&format!("{COMFY}/prompt"))
        .timeout(Duration::from_secs(60))
        .send_json(serde_json::json!({"prompt": graph}))
        .expect("ComfyUI accepted it")
        .into_json()
        .expect("readable");
    let id = queued["prompt_id"]
        .as_str()
        .expect("a prompt id")
        .to_owned();
    println!("queued {id}");

    let mut made: Option<serde_json::Value> = None;
    for _ in 0..600 {
        std::thread::sleep(Duration::from_secs(1));
        let history: serde_json::Value = ureq::get(&format!("{COMFY}/history/{id}"))
            .timeout(Duration::from_secs(30))
            .call()
            .expect("history answered")
            .into_json()
            .expect("readable");
        if let Some(entry) = history.get(&id) {
            made = Some(entry.clone());
            break;
        }
    }
    let made = made.expect("it finished within ten minutes");
    let image = made["outputs"]
        .as_object()
        .expect("outputs")
        .values()
        .filter_map(|node| node.get("images")?.as_array()?.first().cloned())
        .next()
        .expect("something was saved");

    let answer = ureq::get(&format!(
        "{COMFY}/view?filename={}&subfolder={}&type={}",
        image["filename"].as_str().unwrap_or_default(),
        image["subfolder"].as_str().unwrap_or(""),
        image["type"].as_str().unwrap_or("output")
    ))
    .timeout(Duration::from_secs(60))
    .call()
    .expect("the picture came back");
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut answer.into_reader(), &mut bytes).expect("read");
    assert!(bytes.len() > 10_000, "{} bytes", bytes.len());

    let out = std::env::temp_dir().join("epoch-flux-drew.png");
    std::fs::write(&out, &bytes).expect("written");
    println!("drew {} bytes -> {}", bytes.len(), out.display());
}

/// The assembled recipe, with a Z-Image model and one encoder.
///
/// The same three loaders as Flux, one encoder instead of two, and a different string. If this
/// draws, the claim that one recipe covers the families that ship in parts is measured rather
/// than argued.
#[test]
#[ignore = "needs ComfyUI serving on 8188 with Z-Image installed"]
fn the_assembled_recipe_draws_z_image() {
    let info: serde_json::Value = ureq::get(&format!("{COMFY}/object_info"))
        .timeout(Duration::from_secs(60))
        .call()
        .expect("ComfyUI answered")
        .into_json()
        .expect("readable");

    let pick = |node: &str, input: &str, needle: &str| -> Option<String> {
        info[node]["input"]["required"][input][0]
            .as_array()?
            .iter()
            .filter_map(|name| name.as_str())
            .find(|name| name.to_ascii_lowercase().contains(needle))
            .map(str::to_owned)
    };

    let unet = pick("UNETLoader", "unet_name", "z_image").expect("a Z-Image diffusion model");
    let clip = pick("CLIPLoader", "clip_name", "qwen_3_4b").expect("a Qwen3-4B encoder");
    let vae = pick("VAELoader", "vae_name", "z_image").expect("Z-Image's VAE");
    println!(
        "unet {unet}
clip {clip}
vae {vae}"
    );

    let mut ask = Ask::of("unused", "a lighthouse at dusk, painterly");
    ask.loading = Loading::Assembled {
        unet,
        clip: vec![clip],
        // ComfyUI has no `z_image` type: the encoder file is what makes it one, and any type but
        // Flux's reaches the same place (`comfy/sd.py`, read 2026-08-24).
        clip_type: "stable_diffusion".to_owned(),
        vae,
    };
    ask.width = 1024;
    ask.height = 1024;
    // Turbo is distilled: a handful of steps is the whole point of it.
    ask.steps = 8;
    ask.seed = 11;

    let graph = compose(&ask, &Schema::read(&info)).expect("it composed");
    let queued = ureq::post(&format!("{COMFY}/prompt"))
        .timeout(Duration::from_secs(60))
        .send_json(serde_json::json!({"prompt": graph}));
    let queued: serde_json::Value = match queued {
        Ok(said) => said.into_json().expect("readable"),
        Err(ureq::Error::Status(_, said)) => {
            panic!(
                "ComfyUI refused it: {}",
                said.into_string().unwrap_or_default()
            )
        }
        Err(why) => panic!("{why}"),
    };
    let id = queued["prompt_id"]
        .as_str()
        .expect("a prompt id")
        .to_owned();
    println!("queued {id}");

    let mut made: Option<serde_json::Value> = None;
    for _ in 0..600 {
        std::thread::sleep(Duration::from_secs(1));
        let history: serde_json::Value = ureq::get(&format!("{COMFY}/history/{id}"))
            .timeout(Duration::from_secs(30))
            .call()
            .expect("history answered")
            .into_json()
            .expect("readable");
        if let Some(entry) = history.get(&id) {
            made = Some(entry.clone());
            break;
        }
    }
    let made = made.expect("it finished within ten minutes");
    let image = made["outputs"]
        .as_object()
        .expect("outputs")
        .values()
        .filter_map(|node| node.get("images")?.as_array()?.first().cloned())
        .next()
        .unwrap_or_else(|| panic!("nothing was saved: {made}"));

    let answer = ureq::get(&format!(
        "{COMFY}/view?filename={}&subfolder={}&type={}",
        image["filename"].as_str().unwrap_or_default(),
        image["subfolder"].as_str().unwrap_or(""),
        image["type"].as_str().unwrap_or("output")
    ))
    .timeout(Duration::from_secs(60))
    .call()
    .expect("the picture came back");
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut answer.into_reader(), &mut bytes).expect("read");
    assert!(bytes.len() > 10_000, "{} bytes", bytes.len());

    let out = std::env::temp_dir().join("epoch-zimage-drew.png");
    std::fs::write(&out, &bytes).expect("written");
    println!("drew {} bytes -> {}", bytes.len(), out.display());
}

/// A reference picture, prepared into a control map, actually drawn.
///
/// **The unit tests hold the shape; only this holds that ComfyUI runs it.** A `Canny` node filled
/// from the server's declared defaults is a graph that can still be refused — a widget missed, a
/// value out of its range — and that refusal would arrive at render time rather than at the door.
///
/// Why the preparation exists at all was measured by eye, and it does not belong in an assertion:
/// the same seed and the same reference drew unusable noise handed over raw, and drew the
/// reference's own composition through `Canny`.
#[test]
#[ignore = "needs ComfyUI serving on 8188 with a ControlNet installed"]
fn a_prepared_reference_steers_a_real_picture() {
    let info: serde_json::Value = ureq::get(&format!("{COMFY}/object_info"))
        .timeout(Duration::from_secs(60))
        .call()
        .expect("ComfyUI answered")
        .into_json()
        .expect("readable");

    let first = |node: &str, input: &str| {
        info[node]["input"]["required"][input][0][0]
            .as_str()
            .map(str::to_owned)
    };
    let checkpoint = first("CheckpointLoaderSimple", "ckpt_name").expect("a checkpoint");
    let net =
        first("ControlNetLoader", "control_net_name").expect("this ComfyUI lists a ControlNet");
    let reference = first("LoadImage", "image").expect("this ComfyUI has an input picture");
    println!(
        "checkpoint {checkpoint}
controlnet {net}
reference  {reference}"
    );

    // Only what the server actually has — the same filter the panel offers from.
    let offered = epoch_assets::workflow::preparations(&Schema::read(&info));
    assert!(
        !offered.is_empty(),
        "no preparation on this server: {offered:?}"
    );

    let mut ask = Ask::of(&checkpoint, "a stained glass window, vivid colours");
    ask.seed = 7;
    ask.control = vec![epoch_assets::workflow::Steering {
        model: net,
        image: reference,
        prepare: Some(offered[0].clone()),
        strength: 1.0,
        start: 0.0,
        end: 1.0,
    }];

    let schema = Schema::read(&info);
    let graph = compose(&ask, &schema).expect("it composed");

    // The apply node reads what was made of the picture, not the picture.
    let nodes = graph.as_object().expect("a flat map");
    let apply = nodes
        .iter()
        .find(|(_, it)| it["class_type"] == "ControlNetApplyAdvanced")
        .expect("an apply node");
    let reads = apply.1["inputs"]["image"][0].as_str().expect("a wire");
    assert_eq!(nodes[reads]["class_type"], offered[0].as_str());

    // **Accepted, not merely posted.** A refusal comes back as a 400 with the node and the value
    // it did not like, and a status check alone would call that a pass.
    let queued = ureq::post(&format!("{COMFY}/prompt"))
        .timeout(Duration::from_secs(60))
        .send_json(serde_json::json!({"prompt": graph}));
    let queued: serde_json::Value = match queued {
        Ok(answer) => answer.into_json().expect("readable"),
        Err(ureq::Error::Status(_, answer)) => {
            panic!(
                "ComfyUI refused it: {}",
                answer.into_string().unwrap_or_default()
            )
        }
        Err(other) => panic!("{other}"),
    };
    let id = queued["prompt_id"]
        .as_str()
        .expect("a prompt id")
        .to_owned();

    let mut made: Option<serde_json::Value> = None;
    for _ in 0..180 {
        std::thread::sleep(Duration::from_secs(1));
        let history: serde_json::Value = ureq::get(&format!("{COMFY}/history/{id}"))
            .timeout(Duration::from_secs(30))
            .call()
            .expect("history answered")
            .into_json()
            .expect("readable");
        if let Some(entry) = history.get(&id) {
            made = Some(entry.clone());
            break;
        }
    }
    let made = made.expect("it finished within three minutes");
    let image = made["outputs"]
        .as_object()
        .expect("outputs")
        .values()
        .filter_map(|node| node.get("images")?.as_array()?.first().cloned())
        .next()
        .expect("something was saved");
    println!("drew {}", image["filename"].as_str().unwrap_or("?"));
}

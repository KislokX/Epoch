//! Asking a ComfyUI to draw, and waiting for the picture.
//!
//! The half of image generation that knows what ComfyUI is. [`crate::capabilities::draw`] owns
//! what was *asked for* and this owns how it is produced — the same split
//! [`crate::sight`] draws for looking, and the seam a second engine plugs into.
//!
//! ## Three requests, and none of them is a stream
//!
//! Measured 2026-08-21 against the real server:
//!
//! ```text
//! POST /prompt      { prompt, client_id }   -> { prompt_id }
//! GET  /history/{id}                        -> {} until it is finished
//! GET  /view?filename=…&type=output         -> the bytes
//! ```
//!
//! ComfyUI also offers a websocket with progress on it. It is not used: a capability answers
//! once, and a progress bar nothing can show is a dependency for nothing. When long work exists
//! (Phase 12) this is where it attaches.
//!
//! ## The picture is copied out
//!
//! ComfyUI writes into its own output directory, which belongs to ComfyUI: it is cleaned,
//! re-pointed by its settings, and shared by every workflow somebody runs by hand. A Quest's
//! evidence cannot live somewhere Epoch does not own, so the bytes are fetched and written into
//! the vault beside the pictures the user shared — where the Chronicle already knows how to draw
//! them.

use std::time::{Duration, Instant};

use epoch_assets::workflow::Imported;

/// How long to wait for a picture before giving up.
///
/// Generous on purpose: a cold model loads gigabytes before it draws anything — measured at
/// 12.1 s warm on this machine and considerably worse for the first request after a start.
const PATIENCE: Duration = Duration::from_secs(600);

/// How often to ask whether it is done.
///
/// A second is imperceptible against a job measured in seconds, and it costs one request.
const ASK_AGAIN: Duration = Duration::from_secs(1);

/// A running ComfyUI.
pub struct Comfy {
    endpoint: String,
}

impl Comfy {
    pub fn at(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_owned(),
        }
    }

    /// What this server knows how to do, for compiling a workflow against it.
    pub fn schema(&self) -> Result<epoch_assets::workflow::Schema, String> {
        let info: serde_json::Value = ureq::get(&format!("{}/object_info", self.endpoint))
            .timeout(Duration::from_secs(30))
            .call()
            .map_err(|err| format!("ComfyUI did not answer: {err}"))?
            .into_json()
            .map_err(|err| format!("ComfyUI answered with something unreadable: {err}"))?;
        Ok(epoch_assets::workflow::Schema::read(&info))
    }

    /// Hand a picture over, and get back the name the server knows it by.
    ///
    /// **Measured 2026-08-22**, because the shape was not obvious: `POST /upload/image` takes
    /// `multipart/form-data` and answers `{"name": "…", "subfolder": "", "type": "input"}`. That
    /// name is what a `LoadImage` node loads by — Epoch's own filename means nothing there, and
    /// a path from this machine means less: a lent ComfyUI has its own disk.
    ///
    /// The body is written by hand because `ureq` has no multipart and this needs one field and
    /// one file. A crate for that would be a dependency for eleven lines.
    pub fn hand_over(&self, name: &str, bytes: &[u8]) -> Result<String, String> {
        const EDGE: &str = "----epoch-hands-over-a-picture";
        // **CRLF, written as escapes.** Multipart is a wire format and a bare newline is not
        // it: a literal line break in this source compiles perfectly and produces a body that
        // servers reject for reasons they do not explain.
        let head = format!(
            "--{EDGE}\r\nContent-Disposition: form-data; name={Q}image{Q}; filename={Q}{name}{Q}\r\nContent-Type: application/octet-stream\r\n\r\n",
            Q = char::from(34),
        );
        // Overwrite: the name comes from the bytes, so the same picture is the same file, and a
        // second copy under `image (1).png` would be a second name for one thing.
        let tail = format!(
            "\r\n--{EDGE}\r\nContent-Disposition: form-data; name={Q}overwrite{Q}\r\n\r\ntrue\r\n--{EDGE}--\r\n",
            Q = char::from(34),
        );

        let mut body = head.into_bytes();
        body.extend(bytes);
        body.extend(tail.as_bytes());

        let answer: serde_json::Value = ureq::post(&format!("{}/upload/image", self.endpoint))
            .set(
                "Content-Type",
                &format!("multipart/form-data; boundary={EDGE}"),
            )
            .timeout(Duration::from_secs(120))
            .send_bytes(&body)
            .map_err(refused)?
            .into_json()
            .map_err(|err| format!("ComfyUI took the picture and answered oddly: {err}"))?;

        answer["name"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "ComfyUI took the picture and did not say what it called it".to_owned())
    }
    /// Run one workflow and hand back the bytes of what it drew.
    ///
    /// The whole failure is reported, because ComfyUI's refusals are *useful*: it names the node,
    /// the input and the value it did not like. Swallowing that in favour of "generation failed"
    /// would throw away the only thing that explains what to fix.
    pub fn draw(&self, workflow: &Imported) -> Result<(Vec<u8>, String, f32), String> {
        self.draw_graph(&workflow.compiled.nodes)
    }

    /// The same run, from a graph that arrived rather than one compiled here.
    ///
    /// **Split out for the Bridge.** A lent machine is handed the compiled graph over the wire
    /// and runs it against its own loopback ComfyUI, so it has the nodes and not the `Imported`
    /// they came from. Sharing this rather than writing a second queue-and-wait is what keeps
    /// *which picture is the answer* a single fact — the two would otherwise drift, and the
    /// drift would show up as one machine returning a different image from the same job.
    pub fn draw_graph(
        &self,
        nodes: &impl serde::Serialize,
    ) -> Result<(Vec<u8>, String, f32), String> {
        let started = Instant::now();
        let queued: serde_json::Value = ureq::post(&format!("{}/prompt", self.endpoint))
            .timeout(Duration::from_secs(60))
            .send_json(serde_json::json!({
                "prompt": nodes,
                "client_id": "epoch",
            }))
            .map_err(refused)?
            .into_json()
            .map_err(|err| format!("ComfyUI answered with something unreadable: {err}"))?;

        let id = queued["prompt_id"]
            .as_str()
            .ok_or("ComfyUI took the work and did not say what it was called")?
            .to_owned();

        loop {
            let history: serde_json::Value = ureq::get(&format!("{}/history/{id}", self.endpoint))
                .timeout(Duration::from_secs(30))
                .call()
                .map_err(|err| format!("ComfyUI stopped answering: {err}"))?
                .into_json()
                .map_err(|err| format!("ComfyUI answered with something unreadable: {err}"))?;

            if let Some(entry) = history.get(&id) {
                let status = entry["status"]["status_str"].as_str().unwrap_or("unknown");
                if status != "success" {
                    return Err(format!("ComfyUI could not finish it ({status})"));
                }
                let (name, subfolder, kind) = first_image(entry)
                    // A workflow that ran and saved nothing is a workflow with no `SaveImage`,
                    // `SaveVideo` or `SaveAudio` in it. Worth its own sentence: nothing is wrong
                    // with the server.
                    .ok_or("that workflow ran and saved nothing")?;
                let bytes = self.fetch(&name, &subfolder, &kind)?;
                return Ok((bytes, name, started.elapsed().as_secs_f32()));
            }

            if started.elapsed() > PATIENCE {
                return Err(format!(
                    "ComfyUI has been drawing for {} minutes and has not finished",
                    PATIENCE.as_secs() / 60
                ));
            }
            std::thread::sleep(ASK_AGAIN);
        }
    }

    /// Ask this ComfyUI to let go of what it loaded.
    ///
    /// **Measured 2026-08-25, on this machine, with `ponyDiffusionV6XL`:** before a render
    /// 18841 MB of system memory and 11069 MB of video memory were free; after it, 10728 and
    /// 4369. One `POST /free` returned them to 17832 and 10994 — **7.1 GB of RAM and 6.6 GB of
    /// video memory**. So the answer to *is it even using the card* is yes, both, and neither is
    /// given back on its own.
    ///
    /// The same door the local text runtimes have (11.21), and the same discipline: the reply is
    /// an empty 200, which says nothing about whether anything happened, so the claim above comes
    /// from reading `/system_stats` back rather than from the status code.
    ///
    /// Best-effort by construction. A picture that was drawn is drawn; failing to tidy up after
    /// it is not a reason to fail the capability, and the caller has nothing useful to do with
    /// the error.
    pub fn release(&self) {
        let _ = ureq::post(&format!("{}/free", self.endpoint))
            .timeout(Duration::from_secs(30))
            .send_json(serde_json::json!({
                // Both, because they are two different pools and the user watching Task Manager
                // is looking at the first one.
                "unload_models": true,
                "free_memory": true,
            }));
    }

    /// The bytes of one output image.
    fn fetch(&self, name: &str, subfolder: &str, kind: &str) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        ureq::get(&format!("{}/view", self.endpoint))
            .query("filename", name)
            .query("subfolder", subfolder)
            .query("type", kind)
            .timeout(Duration::from_secs(120))
            .call()
            .map_err(|err| format!("ComfyUI drew it and would not hand it over: {err}"))?
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|err| format!("the picture could not be read: {err}"))?;
        if bytes.is_empty() {
            return Err("ComfyUI handed over an empty file".to_owned());
        }
        Ok(bytes)
    }
}

/// The first thing a finished job reports, whatever medium it is.
///
/// First rather than all: a capability answers with one thing, and a workflow that saves several
/// is saving intermediate steps far more often than it is saving several finished results.
///
/// **Two keys, and the second was found by making a sound** (2026-08-28). `SaveImage` reports
/// under `images` and so does `SaveVideo` — measured, with an `animated: [true]` beside it — but
/// `SaveAudio` reports under `audio`. A reader that knew only the first would have watched a
/// perfectly successful render and answered *that workflow ran and saved no picture*.
fn first_image(entry: &serde_json::Value) -> Option<(String, String, String)> {
    entry["outputs"].as_object()?.values().find_map(|out| {
        let saved = out
            .get("images")
            .or_else(|| out.get("audio"))
            .or_else(|| out.get("video"))
            // `SaveGLB` reports under `3d` — measured 2026-08-29, and the fourth key in a row
            // that started as one. Each was found by making the thing rather than by reading.
            .or_else(|| out.get("3d"))?;
        saved.as_array()?.iter().find_map(|image| {
            Some((
                image.get("filename")?.as_str()?.to_owned(),
                image
                    .get("subfolder")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                image
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("output")
                    .to_owned(),
            ))
        })
    })
}

/// ComfyUI's own words when it refuses.
///
/// It says which node, which input and which value — `unet_name: 'x' not in []`, `cfg: 721… bigger
/// than max of 100`. That is the difference between a message somebody can act on and one that
/// only says something went wrong.
fn refused(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(_, response) => match response.into_string() {
            Ok(said) => format!("ComfyUI refused it: {said}"),
            Err(_) => "ComfyUI refused it and would not say why".to_owned(),
        },
        other => format!("ComfyUI could not be reached: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_saved_picture_is_the_answer() {
        let entry = serde_json::json!({
            "outputs": {
                "9": { "images": [
                    { "filename": "epoch_00001_.png", "subfolder": "", "type": "output" }
                ]}
            }
        });
        let (name, subfolder, kind) = first_image(&entry).expect("one picture");
        assert_eq!(name, "epoch_00001_.png");
        assert!(subfolder.is_empty());
        assert_eq!(kind, "output");
    }

    #[test]
    fn a_job_that_saved_nothing_is_not_a_picture() {
        // A workflow with no `SaveImage` ran perfectly and produced no file. Nothing is wrong
        // with the server, and the sentence should say so rather than blaming it.
        let entry = serde_json::json!({ "outputs": { "9": { "text": ["hello"] } } });
        assert!(first_image(&entry).is_none());
    }

    #[test]
    fn a_missing_type_is_an_output_because_that_is_where_pictures_land() {
        let entry = serde_json::json!({
            "outputs": { "9": { "images": [{ "filename": "a.png" }] } }
        });
        let (_, _, kind) = first_image(&entry).expect("one picture");
        assert_eq!(kind, "output");
    }

    /// The measurement behind [`Comfy::release`], runnable.
    ///
    /// It **draws first**, because a server holding nothing has nothing to give back — the first
    /// version of this asserted on a precondition it had not established and failed against a
    /// perfectly healthy ComfyUI. And it reads the **side effect** rather than the status code:
    /// `/free` answers an empty 200 whether or not anything happened, which is the trap LM Studio
    /// set in 11.21.
    ///
    /// Measured on this machine with `ponyDiffusionV6XL`: 10728 MB of system memory free after
    /// the render, 17832 MB after the ask.
    #[test]
    #[ignore = "needs ComfyUI serving on 8188 with at least one checkpoint"]
    fn asking_it_to_let_go_gives_the_memory_back() {
        const HOST: &str = "http://127.0.0.1:8188";
        let comfy = Comfy::at(HOST);
        let ask = |path: String| -> serde_json::Value {
            ureq::get(&format!("{HOST}{path}"))
                .call()
                .expect("ComfyUI answers")
                .into_json()
                .expect("json")
        };
        let free = || {
            ask("/system_stats".to_owned())["system"]["ram_free"]
                .as_u64()
                .expect("ram_free")
        };

        // Whatever this machine actually has. A name written into a test is a name that is wrong
        // on the next computer.
        let offered = ask("/object_info/CheckpointLoaderSimple".to_owned());
        let checkpoint = offered["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"][0][0]
            .as_str()
            .expect("a checkpoint on the shelf")
            .to_owned();

        let graph = serde_json::json!({
            "1": {"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name": checkpoint}},
            "2": {"class_type":"CLIPTextEncode","inputs":{"text":"a lighthouse","clip":["1",1]}},
            "3": {"class_type":"CLIPTextEncode","inputs":{"text":"","clip":["1",1]}},
            "4": {"class_type":"EmptyLatentImage","inputs":{"width":512,"height":512,"batch_size":1}},
            "5": {"class_type":"KSampler","inputs":{"seed":1,"steps":4,"cfg":7.0,
                   "sampler_name":"euler","scheduler":"normal","denoise":1.0,
                   "model":["1",0],"positive":["2",0],"negative":["3",0],"latent_image":["4",0]}},
            "6": {"class_type":"VAEDecode","inputs":{"samples":["5",0],"vae":["1",2]}},
            "7": {"class_type":"SaveImage","inputs":{"filename_prefix":"epochfree","images":["6",0]}},
        });
        comfy.draw_graph(&graph).expect("it draws");

        let held = free();
        comfy.release();
        std::thread::sleep(Duration::from_secs(5));
        let after = free();
        assert!(
            after > held + 512 * 1024 * 1024,
            "asking ComfyUI to let go returned almost nothing: {held} -> {after}"
        );
    }
}

//! A mesh, turned, so the conversation can show it.
//!
//! ## Why a preview and not a viewer
//!
//! Phase 12 has said since ADR-0034 that 3D waits on *something that can display a mesh*, and
//! the World is 2D. That is still true of the Chronicle: it is a webview, and a mesh is not a
//! picture.
//!
//! What changed is that a mesh can be **turned into** a picture. `polyscope-rs` renders headless
//! — no window, no event loop, one frame at a time — so Epoch draws the model from every angle
//! and keeps the result as an animated GIF beside the mesh. Measured on this machine: a 390k-face
//! cow, 24 frames at 384px, in **8.0 s**.
//!
//! ## What this deliberately is not
//!
//! **Not interactive.** `polyscope-rs` can be driven with a mouse, in *its own window* — winit
//! and egui, a second place things live, outside the World. That is the immersion leak this
//! project has a name for, and on Windows winit's event loop wants the main thread that Tauri
//! already owns. Rendering a frame per drag event over IPC measures ~270 ms, which is four
//! frames a second and not a viewer either.
//!
//! So: it turns. Every angle, on its own, in the place the picture would have been.
//!
//! ## The name is the mesh's
//!
//! `<stem>.gif` beside `<stem>.glb`, where the stem is the hash of the mesh's own bytes. That is
//! a statement rather than a coincidence — the preview is *of* that mesh and of nothing else —
//! and it is what lets the Chronicle find one without a second field on the artifact. A mesh
//! whose preview is missing still shows its row, still says a model was made, and still opens.

use std::path::Path;

use glam::{Mat4, UVec3, Vec3};

/// How many angles. A full turn, divided.
///
/// Twenty-four is a second at film rate and about eight seconds of rendering. Fewer stutters;
/// more costs time nobody is watching for.
const FRAMES: u32 = 24;

/// How big, per side. Square, because the thing being turned has no aspect ratio of its own.
///
/// 384 rather than the Chronicle's 360: a GIF is scaled by the browser anyway, and the extra
/// pixels are a rounding allowance rather than a decision.
const SIDE: u32 = 384;

/// How many faces one registered structure may hold.
///
/// **Measured, not chosen.** wgpu refuses a bind group whose buffer range passes
/// `max_*_buffer_binding_size` — 134 217 728 bytes here — and polyscope's per-face buffer is
/// 48 bytes wide, so the true ceiling is 2 796 202. Two million leaves room for a driver that
/// reports a smaller limit without making the parts small enough to cost frames.
const PER_PART: usize = 2_000_000;

/// Turn a mesh into an animated preview, and answer where it landed.
///
/// **Best-effort by construction.** A mesh that was made is made; failing to draw a picture of it
/// is not a reason to fail the work, and the Chronicle already renders a row with no preview.
/// Every failure here is `None` and the caller carries on.
///
/// ## And a contract is only kept if the failure can reach it
///
/// That paragraph was written before a failure arrived that `?` cannot see. `fine` produced a
/// 2 805 800-face cow, wgpu answered
/// `Buffer binding 2 range 134678400 exceeds max_*_buffer_binding_size limit 134217728`, and it
/// answered it by **panicking** — so the turn did not fall back to no preview, it stopped. The
/// model sat finished in the vault and the card read WAITING for twenty minutes.
///
/// > **A best-effort promise made with `?` covers the errors that are returned and none of the
/// > ones that are raised.** The panic is caught here, so the sentence above is true again.
pub(crate) fn preview(mesh: &Path, beside: &Path) -> Option<std::path::PathBuf> {
    // The whole render, behind a net. Everything below can fail by returning; wgpu can also fail
    // by panicking, and the caller must not be able to tell the difference.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(mesh, beside))).ok()?
}

fn draw(mesh: &Path, beside: &Path) -> Option<std::path::PathBuf> {
    let (vertices, faces) = read_glb(mesh)?;
    if vertices.is_empty() || faces.is_empty() {
        return None;
    }

    // **Started once per process, and the second call is not a failure.** Polyscope keeps its
    // context in a `OnceLock`; `init` answers `Err` when it is already there. Bailing on that
    // meant the *first* model of a session turned and every one after it did not — and the
    // Chronicle, finding no preview, said the mesh was no longer in the vault. It was.
    //
    // Measured in the window: one model, then a second, 37.3 s apart.
    let _ = polyscope_rs::init();

    // A name of its own, because the registry is process-wide and one before it may still be in
    // there. Two meshes under one name is a preview of whichever won.
    // **Emptied first.** The registry is process-wide and outlives this call, so a mesh left in
    // it from the model before is drawn behind this one. `remove_all_of_type` is the door; there
    // is no `remove_surface_mesh` in 0.5.10.
    polyscope_rs::with_context_mut(|ctx| ctx.registry.remove_all_of_type("SurfaceMesh"));

    // **Registered in pieces, because one buffer has a ceiling and a mesh does not.** The
    // measured limit is 134 217 728 bytes at 48 bytes a face — 2 796 202 of them — and the
    // `fine` cow came to 2 805 800, over by a third of a percent. Splitting the faces and
    // registering each part draws the *whole* surface: the alternative, dropping triangles to
    // fit, is a preview with holes in it, which is a picture of a different model.
    //
    // Measured: 2 parts, 24 frames, 15.0 s, against 8.0 s for a 390k-face one in a single part.
    let names: Vec<String> = faces
        .chunks(PER_PART)
        .enumerate()
        .map(|(at, part)| {
            let name = format!("preview-{at}");
            polyscope_rs::register_surface_mesh(&name, vertices.clone(), part.to_vec());
            name
        })
        .collect();

    // **The mesh turns, not the camera.** `set_surface_mesh_transform` is public in 0.5.10 and
    // no camera setter is; the picture is the same either way.
    let mut shots = Vec::with_capacity(FRAMES as usize);
    for frame in 0..FRAMES {
        let angle = std::f32::consts::TAU * (frame as f32) / (FRAMES as f32);
        for name in &names {
            polyscope_rs::set_surface_mesh_transform(name, Mat4::from_rotation_y(angle));
        }
        let raw = polyscope_rs::render_to_image(SIDE, SIDE).ok()?;
        shots.push(image::RgbaImage::from_raw(SIDE, SIDE, raw)?);
    }

    // A GIF, because it plays in an `<img>` and needs no encoder, no muxer and no new element in
    // the conversation — the picture path that already exists carries it unchanged. A clay
    // render is grey, so 256 colours is not the limit here.
    let at = beside.join(format!(
        "{}.gif",
        mesh.file_stem()?.to_str().unwrap_or_default()
    ));
    let file = std::fs::File::create(&at).ok()?;
    let mut gif = image::codecs::gif::GifEncoder::new_with_speed(std::io::BufWriter::new(file), 10);
    gif.set_repeat(image::codecs::gif::Repeat::Infinite).ok()?;
    for shot in shots {
        gif.encode_frame(image::Frame::from_parts(
            shot,
            0,
            0,
            image::Delay::from_numer_denom_ms(1000 / FRAMES, 1),
        ))
        .ok()?;
    }
    drop(gif);
    Some(at)
}

/// Every triangle in a glTF binary, as one mesh.
///
/// **Flattened deliberately.** Polyscope draws a structure; a preview of *the thing that was
/// made* is one structure, and a file whose parts arrive as three primitives is still one cow.
fn read_glb(path: &Path) -> Option<(Vec<Vec3>, Vec<UVec3>)> {
    let (document, buffers, _) = gltf::import(path).ok()?;
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut faces: Vec<UVec3> = Vec::new();
    for mesh in document.meshes() {
        for part in mesh.primitives() {
            let read = part.reader(|b| Some(&buffers[b.index()]));
            let base = vertices.len() as u32;
            let Some(positions) = read.read_positions() else {
                continue;
            };
            for xyz in positions {
                vertices.push(Vec3::from(xyz));
            }
            match read.read_indices() {
                Some(indices) => {
                    let flat: Vec<u32> = indices.into_u32().collect();
                    for tri in flat.chunks_exact(3) {
                        faces.push(UVec3::new(base + tri[0], base + tri[1], base + tri[2]));
                    }
                }
                // No index buffer: the positions are the triangles, in order.
                None => {
                    let count = vertices.len() as u32 - base;
                    for at in (0..count.saturating_sub(2)).step_by(3) {
                        faces.push(UVec3::new(base + at, base + at + 1, base + at + 2));
                    }
                }
            }
        }
    }
    Some((vertices, faces))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file that is not a mesh is `None`, not a panic and not an empty GIF.
    #[test]
    fn something_that_is_not_a_mesh_makes_no_preview() {
        let dir = std::env::temp_dir().join(format!("epoch-turn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let not = dir.join("a1b2.glb");
        std::fs::write(&not, b"this is not a glb").expect("it writes");
        assert_eq!(preview(&not, &dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A mesh too big for one buffer still turns.** `#[ignore]` - it needs a real mesh and a
    /// card.
    ///
    /// The `fine` surface produced 2 805 800 faces and wgpu panicked at 2 796 202. Before the
    /// split this call took the whole turn down with it; the assertion is that it answers a
    /// picture, and the `catch_unwind` above is why it can only ever answer or say no.
    #[test]
    #[ignore = "needs a 2.8M-face .glb and a graphics card"]
    fn a_mesh_past_one_buffer_still_turns() {
        let mesh =
            std::path::Path::new(r"C:\Users\someone\Epoch\BUILD\vault\media\b631b9e5908628ab.glb");
        if !mesh.is_file() {
            println!("no big mesh to hand; skipped");
            return;
        }
        let (_, faces) = read_glb(mesh).expect("it reads");
        assert!(faces.len() > PER_PART, "the point is that it does not fit");
        let dir = std::env::temp_dir().join("epoch-turn-big");
        std::fs::create_dir_all(&dir).expect("a folder");
        let at = preview(mesh, &dir).expect("it turns rather than panicking");
        assert!(std::fs::metadata(&at).unwrap().len() > 1000);
    }

    /// **The preview is named after the mesh**, which is what lets the Chronicle find it without
    /// a second field on the artifact. `#[ignore]` — it needs a real mesh and a GPU.
    #[test]
    #[ignore = "needs a real .glb and a graphics card"]
    fn a_real_mesh_turns_and_lands_beside_itself() {
        let mesh = std::path::Path::new(
            r"C:\Users\someone\AppData\Local\Temp\claude\C--Users-someone-Downloads-Proyectos-progra-Epoch\0150207a-ad30-45c4-b94a-d761168b4a5e\scratchpad\cow.glb",
        );
        if !mesh.is_file() {
            println!("no mesh to hand; skipped");
            return;
        }
        let dir = std::env::temp_dir().join("epoch-turn-real");
        std::fs::create_dir_all(&dir).expect("a folder");
        let at = preview(mesh, &dir).expect("it turns");
        assert_eq!(at.file_name().unwrap(), "cow.gif");
        assert!(std::fs::metadata(&at).unwrap().len() > 1000);
        println!("wrote {}", at.display());
    }
}

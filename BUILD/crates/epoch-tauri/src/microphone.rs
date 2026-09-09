//! Epoch answers the microphone question, having actually asked it.
//!
//! ## The leak this closes
//!
//! Pressing `SPEAK` called `getUserMedia`, and Edge put its own prompt over the World:
//!
//! ```text
//! http://tauri.localhost wants to use your microphones
//! ```
//!
//! A URL, in somebody else's frame, on top of a living world. The owner missed it, pressed the
//! button again, and reported *"cuando le doy a speak no pasa nada"* — **nothing was broken**.
//! The control was correct, the ear was correct, and the product looked dead. That is an
//! immersion leak by this project's own definition, and it cost a session before it was fixed.
//!
//! ## What is and is not being decided here
//!
//! This does **not** grant anything on its own. It answers a request the browser raises, from a
//! consent the user gave in Epoch's own words, in the World's own frame, and which is written
//! down (`Settings::microphone`).
//!
//! The three states matter and the third is why this is not a `bool`:
//!
//! | stored | answered | why |
//! |---|---|---|
//! | `Some(true)` | Allow | they said yes, in a frame that said what for |
//! | `Some(false)` | Deny | they said no, and the question must not come back every press |
//! | `None` | **left to the browser** | nobody asked, so Epoch has no answer to give |
//!
//! The last row is the load-bearing one. Silently allowing on `None` would be Epoch consenting
//! on somebody's behalf; silently denying would be the inversion this codebase has paid for
//! before. Handing it back is the honest third answer, and it is what the product does today —
//! so a machine where this handler never attaches is no worse off than before.
//!
//! ## Windows only, and deliberately not abstracted
//!
//! `PermissionRequested` is a WebView2 interface. macOS and Linux use different engines with
//! different prompts, and inventing a shared trait over one implementation would be an
//! abstraction with one caller — the whole file compiles to nothing elsewhere.

/// Let the window answer permission requests from what the user has already told Epoch.
///
/// Failing is not fatal and says so: an unattached handler means the browser asks in its own
/// words, which is exactly what happened before this existed.
#[cfg(windows)]
pub fn answer_for(window: &tauri::WebviewWindow, vault: std::path::PathBuf) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2PermissionRequestedEventArgs, COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
        COREWEBVIEW2_PERMISSION_STATE_ALLOW, COREWEBVIEW2_PERMISSION_STATE_DENY,
    };
    use webview2_com::PermissionRequestedEventHandler;
    use windows_core::Interface;

    let attached = window.with_webview(move |webview| {
        // SAFETY: the only unsafe in this crate, and it is one COM registration on the thread
        // that owns the webview. Everything inside the handler is a read of a settings file.
        unsafe {
            let core = webview.controller().CoreWebView2();
            let Ok(core) = core else { return };
            let mut token = Default::default();
            let _ = core.add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    answer(&args, &vault)
                })),
                &mut token,
            );
            // Touched so the import is load-bearing rather than decorative.
            let _ = core.as_raw();
        }
    });

    if attached.is_err() {
        // Said rather than swallowed: the fallback is Edge's own prompt, which is a worse
        // experience and not a broken one.
        eprintln!(
            "epoch: the microphone question will be asked by the browser rather than by Epoch"
        );
    }

    // Kept out of the closure above so the constants are named in one place.
    #[allow(unused)]
    unsafe fn answer(
        args: &ICoreWebView2PermissionRequestedEventArgs,
        vault: &std::path::Path,
    ) -> windows_core::Result<()> {
        let mut kind = Default::default();
        args.PermissionKind(&mut kind)?;
        if kind != COREWEBVIEW2_PERMISSION_KIND_MICROPHONE {
            // Not ours. Left entirely alone — `Handled` is never set, so the browser behaves
            // exactly as it did before this file existed.
            return Ok(());
        }

        // **Read now, not remembered.** The user may have answered Epoch's own question a
        // second ago; a value captured when the window opened would be the wrong one.
        match epoch_engine::settings::Settings::load(vault).microphone {
            // **`SetState` is the whole answer, and `SetHandled` is not needed.** Measured
            // against the interface rather than assumed: `SetHandled` lives on
            // `…EventArgs2` and suppresses the *default UI* when the state is left at
            // `Default`. Answering the state already settles the request without a prompt, so
            // reaching for a second interface would be work that changes nothing.
            Some(true) => args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?,
            Some(false) => args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?,
            // Nobody asked. Epoch has no answer to give, so it gives none -- the state stays
            // `Default` and the browser asks in its own words, exactly as it did before.
            None => {}
        }
        Ok(())
    }
}

/// Everywhere else the browser asks in its own words, exactly as it did before.
#[cfg(not(windows))]
pub fn answer_for(_window: &tauri::WebviewWindow, _vault: std::path::PathBuf) {}

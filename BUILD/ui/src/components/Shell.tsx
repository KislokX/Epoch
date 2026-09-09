import { useEffect, useRef, useState } from "react";

/**
 * The edge of the application, where a browser stops being invisible.
 *
 * Epoch runs in a webview, and a webview brings a browser's furniture with it: *Inspect*,
 * *Save as*, *Print*, *Share*, *Send to your devices*. None of them are wrong. All of them are
 * an **immersion leak** — the thing that works correctly and still reminds you that the living
 * world is a web page in a frame. `CLAUDE.md` names that as its own category of defect, to be
 * fixed before the next feature, precisely because it is cheap now and compounds later.
 *
 * Two things this deliberately does **not** do:
 *
 * **It does not simply swallow the right button.** Back and Refresh are the two entries that
 * belong to *this* application rather than to the browser it happens to sit in, and a World that
 * answers a right-click with nothing at all is its own small wrongness. So the native menu is
 * replaced rather than removed, and it is drawn in the World's own palette.
 *
 * **It keeps Copy when there is something selected.** The chat is text people quote from;
 * removing the native menu without putting Copy back would take away a real capability to fix
 * a cosmetic one.
 */
export function Shell({ children }: { children: React.ReactNode }) {
  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    selection: string;
    /**
     * The field the menu was opened over, if it was one.
     *
     * Captured at open time rather than read when Paste is clicked: opening the menu moves
     * focus, so by the time anybody presses anything `document.activeElement` is the button
     * they just pressed and the field they meant is gone.
     */
    into: HTMLInputElement | HTMLTextAreaElement | null;
  } | null>(null);
  const panel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onMenu = (event: MouseEvent) => {
      event.preventDefault();
      const over = event.target as HTMLElement | null;
      const field = over?.closest("input, textarea") as
        | HTMLInputElement
        | HTMLTextAreaElement
        | null;
      setMenu({
        x: event.clientX,
        y: event.clientY,
        selection: window.getSelection()?.toString() ?? "",
        into: field && !field.disabled && !field.readOnly ? field : null,
      });
    };

    /*
      The shortcuts that open the same furniture by another door. Print and Save produce a
      browser dialog over the World; the developer tools are a window into the frame Epoch
      exists to hide.

      Reload is deliberately left alone — the user asked to keep it, and it is genuinely useful
      while the World is being built.
    */
    const onKey = (event: KeyboardEvent) => {
      const combo = event.ctrlKey || event.metaKey;
      const blocked =
        (combo && ["p", "s"].includes(event.key.toLowerCase())) ||
        event.key === "F12" ||
        (combo && event.shiftKey && ["i", "j", "c"].includes(event.key.toLowerCase()));
      if (blocked) event.preventDefault();
    };

    // Capture, so a component that stops propagation of its own clicks cannot leave the native
    // menu reachable in one corner of the World and not another.
    window.addEventListener("contextmenu", onMenu, true);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("contextmenu", onMenu, true);
      window.removeEventListener("keydown", onKey, true);
    };
  }, []);

  // Any click, any scroll, Escape: the menu is a momentary thing and should never be something
  // the user has to dismiss deliberately.
  useEffect(() => {
    if (!menu) return;
    /*
      A click *inside* the menu is not a click away from it.

      Without this check the menu was entirely dead: the dismiss listener runs on `pointerdown`
      in the capture phase, which reaches `window` before the event ever reaches the button, so
      the menu closed, the button unmounted, and the `click` that would have run the action was
      never dispatched. Every entry looked broken; REFRESH was simply the one whose failure was
      impossible to miss.
    */
    const away = (event: Event) => {
      if (panel.current?.contains(event.target as Node)) return;
      setMenu(null);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenu(null);
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("wheel", away, true);
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("wheel", away, true);
      window.removeEventListener("keydown", key, true);
    };
  }, [menu]);

  return (
    <>
      {children}
      {menu && (
        <div
          ref={panel}
          className="shellmenu"
          role="menu"
          // Kept inside the window: a menu opened near the right edge would otherwise open
          // partly outside it, which is the one place a pixel-art panel cannot follow.
          style={{
            left: Math.min(menu.x, window.innerWidth - 168),
            top: Math.min(menu.y, window.innerHeight - 96),
          }}
        >
          {menu.selection && (
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                void navigator.clipboard.writeText(menu.selection);
                setMenu(null);
              }}
            >
              COPY
            </button>
          )}
          {/*
            Offered only over a field that can take it. A Paste that is always present and
            usually does nothing teaches the user to stop trusting the menu.

            The insertion is written by hand rather than left to `document.execCommand`, which
            is deprecated, and the native `paste` event cannot be synthesised with a payload.
            Setting `value` directly would also skip React's onChange, so the input event is
            dispatched — otherwise the text would appear and the component would not know.
          */}
          {menu.into && (
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                const field = menu.into;
                setMenu(null);
                if (!field) return;
                void navigator.clipboard.readText().then((text) => {
                  const from = field.selectionStart ?? field.value.length;
                  const to = field.selectionEnd ?? from;
                  const next = field.value.slice(0, from) + text + field.value.slice(to);
                  const setter = Object.getOwnPropertyDescriptor(
                    field instanceof HTMLTextAreaElement
                      ? HTMLTextAreaElement.prototype
                      : HTMLInputElement.prototype,
                    "value",
                  )?.set;
                  setter?.call(field, next);
                  field.dispatchEvent(new Event("input", { bubbles: true }));
                  field.focus();
                  field.setSelectionRange(from + text.length, from + text.length);
                });
              }}
            >
              PASTE
            </button>
          )}
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setMenu(null);
              window.history.back();
            }}
          >
            BACK
          </button>
          <button type="button" role="menuitem" onClick={() => window.location.reload()}>
            REFRESH
          </button>
        </div>
      )}
    </>
  );
}

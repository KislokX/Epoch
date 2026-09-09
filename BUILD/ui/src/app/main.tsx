import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

/*
 * Fonts, self-hosted.
 *
 * The reference design pulled these from the Google Fonts CDN. A desktop application must not
 * reach the network to render its own first frame — offline it would silently fall back, and
 * the bridge would arrive in Arial. Bundling also keeps the licence story simple: all four
 * are OFL, which is what CONTENT_PHILOSOPHY.md requires of anything that ships.
 *
 * Only the weights actually used, so the bundle carries no fonts nobody reads.
 */
import "@fontsource/press-start-2p/400.css";
import "@fontsource/vt323/400.css";
/* The HUD's body face: a pixel sans, so prose inside the World is not a web font in a game. */
import "@fontsource/pixelify-sans/400.css";
import "@fontsource/pixelify-sans/500.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-sans/400-italic.css";

import { App } from "./App";
import { Salvage } from "../components/Salvage";
import { Shell } from "../components/Shell";
import { SfxGate } from "../components/SfxGate";
import "./boot.css";
import "./world.css";
import "./hud.css";
import "./launcher.css";
import "./editor.css";

const root = document.getElementById("world-root");
if (!root) throw new Error("missing #world-root");

createRoot(root).render(
  <StrictMode>
    {/*
      The floor under "the World always renders" (Build From Life, rule 1). Without it, one
      badly shaped field in one projection unmounts everything and Epoch becomes a black
      rectangle with no way to find out why.
    */}
    <Salvage>
      {/* Outside `App` so the guard is in place even on a frame that failed to render. */}
      <Shell>
        <SfxGate />
        <App />
      </Shell>
    </Salvage>
  </StrictMode>,
);

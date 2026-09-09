/**
 * THE WORLD EDITOR — where a World is authored (ADR-0028).
 *
 * ## What this screen is
 *
 * Not a settings dialog over the World. A console: a tool rail, the World itself in the middle,
 * the crew on one side, whatever is selected on the other, and instruments along the bottom.
 * The user learns one gesture per tool and the World is the surface for all of them.
 *
 * That structure came from a reference design the author supplied. What survived is its
 * *interaction model* — modes, a selection-driven inspector, editing on the canvas, residence
 * as a drag. What did not survive is its canvas: it drew seven hardcoded building types with
 * hand-written pixel painters, which is exactly the mistake ADR-0016 exists to prevent. A World
 * Pack resolves what a building looks like; a frontend that knows what a "keep" is would be the
 * same error as a frontend that knows Ollama has `num_ctx`.
 *
 * So the middle of this console is the **same renderer the running World uses** — same
 * projection, same Places, same marks, same camera. A second renderer would be free to disagree
 * with the World about what the World looks like, and you would be arranging a building that
 * changes the moment you press PLAY.
 *
 * ## The World is stopped while this is open
 *
 * Entering freezes the Workspace, and freezing is refused while anybody is working — with the
 * refusal naming who and what, because a message the user cannot act on is a wall. That
 * strictness removes a whole class of question rather than answering it: no handoff can resolve
 * against a map being rearranged, because no handoff is running.
 *
 * The editor never interrupts work. It waits for it.
 *
 * ## It edits the file it reads
 *
 * Every action writes `places.toml` (or the character's own file) and then re-reads. There is
 * no layout held here that the vault could disagree with — an editor over the vault, never a
 * parallel store (ADR-0023). Which is also why the open World picks changes up with no new
 * mechanism, and why there is no SAVE button: there is nothing unsaved.
 *
 * ## Cold instruments, never invented ones
 *
 * Half of the reference design describes subsystems Epoch has not built. Those panels keep
 * their frame, lose their light, and name what will fill them. The one place this mattered most
 * was UNDO: the design's popped a label off a list and changed nothing, which is the worst kind
 * of instrument — one that appears to work. It is wired to a real timeline of map snapshots.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { ImageDrop } from "../../components/ImageDrop";
import { Minimap } from "../../components/Minimap";
import { WorldMap } from "../../components/WorldMap";
import { useCamera } from "../../experience/useCamera";
import { forgetSkin, useSkin } from "../../experience/useSkin";
import { refreshSounds, useSounds } from "../../experience/useSounds";
import type { CharacterView, PlaceView, WorldMap as VaultMap, MapView } from "../../ipc/contracts";
import { CrewPanel } from "./CrewPanel";
import { EditorFailureNotice } from "./EditorFailureNotice";
import { Inspector } from "./Inspector";
import { hintFor, ModeRail } from "./ModeRail";
import { StatusBar } from "./StatusBar";
import { TopBar } from "./TopBar";
import { ALL_LAYERS, type Layers, type StageEditing } from "./stage";
import { useEditor } from "./useEditor";

interface WorldEditorProps {
  readonly worldName: string;
  readonly places: readonly PlaceView[];
  readonly characters: readonly CharacterView[];
  readonly map: MapView | null;
  /** Which model each character resolved to. Measured; `null` where nothing answered. */
  readonly minds: Readonly<Record<string, string | null>>;
  /** Start the World again. */
  readonly onPlay: () => void;
  /** Leave the World entirely. */
  readonly onBridge: () => void;
  /** Re-read the World after a change, so what is drawn stays true. */
  readonly onChanged: () => void;
}

export function WorldEditor({
  worldName,
  places,
  characters,
  map,
  minds,
  onPlay,
  onBridge,
  onChanged,
}: WorldEditorProps) {
  const editor = useEditor(onChanged);
  const [vault, setVault] = useState<VaultMap | null>(null);
  const [unreachable, setUnreachable] = useState<readonly string[]>([]);
  const [roadFrom, setRoadFrom] = useState<string | null>(null);
  /** Set only while re-tracing an existing road. New roads still finish at the clicked building. */
  const [roadTo, setRoadTo] = useState<string | null>(null);
  const [roadVia, setRoadVia] = useState<readonly (readonly [number, number])[]>([]);
  const [showResidences, setShowResidences] = useState(true);
  const [layers, setLayers] = useState<Layers>(ALL_LAYERS);
  /** The land its owner painted. Its own fetch: megabytes, never in the per-tick projection. */
  const [land, setLand] = useState<string | null>(null);
  const [cursor, setCursor] = useState<{ x: number; y: number } | null>(null);
  const [naming, setNaming] = useState<{ id: string; value: string } | null>(null);

  /*
    The vault's own view of the map — roads and what the user named — alongside the projection.
    Both come from the Engine and neither is derived from the other here.
  */
  const reread = useCallback(async () => {
    setVault(await invoke<VaultMap>("get_map"));
    setUnreachable(await invoke<string[]>("unreachable_places"));
    setLand(await invoke<string | null>("world_land"));
  }, []);

  useEffect(() => {
    void reread();
  }, [reread, places]);

  const extent = useMemo(
    () => (map ? { width: map.width, height: map.height } : null),
    [map],
  );
  /** Where the camera opens: the largest Place, as the running World does. */
  const arrival = useMemo(() => {
    let widest: { x: number; y: number; footprint: number } | null = null;
    for (const place of places) {
      const at = place.placement;
      if (!at) continue;
      if (!widest || at.footprint > widest.footprint) {
        widest = { x: at.x, y: at.y, footprint: at.footprint };
      }
    }
    return widest;
  }, [places]);

  const camera = useCamera(extent, arrival);

  /* What this World's windows look like today, so LOOK opens on its own numbers rather
     than on a guess that would silently undo them. */
  const skins = useSkin();
  const sounds = useSounds();

  /** The middle of the land, where a new building lands when nobody said where. */
  const centre = map ? { x: map.width / 2, y: map.height / 2 } : null;

  /** Every edit: run it, re-read the vault, re-read the World. */
  const act = (label: string, run: () => Promise<unknown>) =>
    editor.change(label, async () => {
      await run();
      await reread();
      /*
        **And forget what the World looks and sounds like, so the next reader asks again.**

        Both are cached for a session on purpose: `Frame` is used in dozens of places and a hook
        that fetched per window would put dozens of IPC calls on the first frame. That is right
        while a World is only ever *read*. The moment it can be *edited*, a cache that never
        expires is a change the author cannot see — and a sound they cannot hear until Epoch
        restarts is the cold instrument of editors.

        Cheap, and only on an edit: nothing re-asks while somebody is looking around.
      */
      forgetSkin();
      refreshSounds();
    });

  /** Paint the land. Shared: the empty World offers it, and so does the TERRAIN panel. */
  const paintLand = async (image: string | null) => {
    const ok = await act(image ? "painted the land" : "stripped the land", () =>
      invoke("set_world_land", { image }),
    );
    return ok ? null : "That could not be saved.";
  };

  const build = (x: number, y: number) => {
    void editor
      .change("built a place", async () => {
        const id = await invoke<string>("add_place", { name: "New Place", x, y });
        await reread();
        setNaming({ id, value: "New Place" });
      })
      .then(() => editor.setMode("select"));
  };

  const cancelRoad = () => {
    setRoadFrom(null);
    setRoadTo(null);
    setRoadVia([]);
  };

  const saveRoad = () => {
    if (!roadFrom || !roadTo) return;
    const from = roadFrom;
    const to = roadTo;
    const via = roadVia;
    void act("re-traced a road", () => invoke("connect_places", { from, to, joined: true, via }))
      .then((saved) => {
        if (saved) cancelRoad();
      });
  };

  const stage: StageEditing = {
    mode: editor.mode,
    layers,
    selection: editor.selection,
    roadFrom,
    roadTo,
    roadVia,
    showResidences,
    onSelect: editor.select,
    onCursor: setCursor,
    onMovePlace: (id, x, y) => void act("moved a place", () => invoke("move_place", { id, x, y })),
    onBuildAt: build,
    onRoadPick: (id) => {
      // First click picks an end; second joins them; clicking the same one twice cancels.
      // A road to itself is refused by the Engine, but saying so here would be a refusal for
      // something the user obviously meant as "never mind".
      // Re-tracing has both ends already. The user edits its corners on empty land and commits
      // explicitly, so clicking a building can never silently change which two places it joins.
      if (roadTo) return;
      if (roadFrom === id) {
        cancelRoad();
        return;
      }
      if (!roadFrom) {
        setRoadFrom(id);
        setRoadVia([]);
        return;
      }
      const from = roadFrom;
      setRoadFrom(null);
      const via = roadVia;
      setRoadVia([]);
      void act("paved a road", () =>
        invoke("connect_places", { from, to: id, joined: true, via }),
      );
    },
    onRoadCorner: ({ x, y }) => {
      if (!roadFrom) return;
      setRoadVia((points) => {
        const last = points.at(-1);
        return last && last[0] === x && last[1] === y ? points : [...points, [x, y]];
      });
    },
    onAssignHome: (characterId, placeId) =>
      void act("moved somebody home", () =>
        invoke("set_character_home", { characterId, placeId }),
      ),
  };

  return (
    <div className="wed">
      <TopBar editor={editor} worldName={worldName} onPlay={onPlay} onBridge={onBridge} />

      <div className="wed__body">
        <ModeRail
          mode={editor.mode}
          onMode={(m) => {
            setRoadFrom(null);
            setRoadTo(null);
            setRoadVia([]);
            editor.setMode(m);
          }}
        />

        <div className="wed__left">
          <CrewPanel
            editor={editor}
            crew={characters}
            places={places}
            minds={minds}
            onAssign={(characterId, placeId) =>
              void act("moved somebody home", () =>
                invoke("set_character_home", { characterId, placeId }),
              )
            }
          />
        </div>

        <div className="pxpanel wed__centre">
          <div
            className="wed__stage"
            ref={camera.containerRef}
            onContextMenu={(event) => event.preventDefault()}
          >
            {map && camera.viewBox ? (
              <WorldMap
                map={map}
                places={places}
                characters={characters}
                viewBox={camera.viewBox}
                camera={camera.camera}
                surfaceRef={camera.surfaceRef}
                isDragging={camera.isDragging}
                isSettled={camera.isSettled}
                onPointerDown={camera.onPointerDown}
                onPointerMove={camera.onPointerMove}
                onPointerUp={camera.onPointerUp}
                visiting={null}
                hovering={null}
                onVisit={() => {}}
                onHover={() => {}}
                onLeave={() => {}}
                land={land}
                editing={stage}
              />
            ) : (
              /*
                No land at all — a World nobody has started. The editor still opens, because
                this is exactly where somebody is about to build their first thing.

                It used to say "use PLACE", which was impossible: with no extent there is no
                surface to click and no coordinate to build at. The way in is the land, and its
                size becomes the World's size — so this points at the thing that actually works.
              */
              <div
                className="pxinset"
                style={{ height: "100%", display: "grid", placeItems: "center", padding: 24 }}
              >
                <div style={{ maxWidth: 340, textAlign: "center" }}>
                  <div className="pxtitle" style={{ marginBottom: 8 }}>
                    An empty World
                  </div>
                  <p className="pxnote" style={{ marginBottom: 12 }}>
                    Give it ground to stand on. The picture you choose is the World, at the size
                    you drew it — one pixel, one world unit — and everything you build sits on it.
                  </p>
                  <ImageDrop
                    label="Choose the land"
                    className="pxbtn pxbtn--gold"
                    onChoose={paintLand}
                  />
                </div>
              </div>
            )}

            <div className="pxpanel wed__hint">
              <span className="pxtitle">{editor.mode} mode</span>
              <span>{hintFor(editor.mode, roadFrom, roadVia.length, roadTo)}</span>
            </div>

            {map && (
              <div className="pxpanel wed__minimap">
                <div className="pxtitle" style={{ marginBottom: 4 }}>
                  System Map
                </div>
                <Minimap
                  map={map}
                  places={places}
                  view={camera.view}
                  onLookAt={camera.lookAtWorld}
                  land={land}
                  characters={characters}
                  inline
                />
              </div>
            )}

            <div className="wed__tools">
              {roadFrom && (
                <>
                  <button
                    type="button"
                    className="pxbtn"
                    disabled={roadVia.length === 0}
                    onClick={() => setRoadVia((points) => points.slice(0, -1))}
                  >
                    Undo corner
                  </button>
                  {roadTo && (
                    <button type="button" className="pxbtn pxbtn--gold" onClick={saveRoad}>
                      Save route
                    </button>
                  )}
                  <button
                    type="button"
                    className="pxbtn"
                    onClick={cancelRoad}
                  >
                    Cancel road
                  </button>
                </>
              )}
              <button
                type="button"
                className={`pxbtn${showResidences ? " pxbtn--on" : ""}`}
                title="Draw a line from everybody who is out to the building they live in"
                onClick={() => setShowResidences((on) => !on)}
              >
                ⌂ Residences
              </button>
              <button
                type="button"
                className="pxbtn"
                onClick={() => arrival && camera.lookAtWorld({ x: arrival.x, y: arrival.y })}
              >
                Reset view
              </button>
            </div>
          </div>
        </div>

        <div className="wed__right">
          <EditorFailureNotice failure={editor.failed} onDismiss={editor.clearFailure} />
          <Inspector
            editor={editor}
            places={places}
            crew={characters}
            map={vault}
            centre={centre}
            layers={layers}
            onLayers={setLayers}
            hasLand={Boolean(vault?.land)}
            onLand={paintLand}
            look={{
              current: skins["ui.frame.window"],
              onSave: async (image, corner, repeat, scale) => {
                const ok = await act("dressed the windows", () =>
                  invoke("set_world_skin", {
                    concept: "ui.frame.window",
                    image,
                    corner,
                    repeat,
                    scale,
                  }),
                );
                return ok ? null : "That could not be saved.";
              },
              onClear: async () => {
                const ok = await act("undressed the windows", () =>
                  invoke("set_world_skin", {
                    concept: "ui.frame.window",
                    image: null,
                    corner: [0, 0, 0, 0],
                    repeat: "stretch",
                    scale: 1,
                  }),
                );
                return ok ? null : "That could not be undone.";
              },
              sounds,
              onSound: async (concept, audio) => {
                const ok = await act(audio ? "gave the World a voice" : "took a voice back", () =>
                  invoke("set_world_sound", { concept, audio }),
                );
                return ok ? null : "That could not be saved.";
              },
            }}
            onArt={async (id, image) => {
              const ok = await act(image ? "redrew a place" : "cleared a drawing", () =>
                invoke("set_place_art", { id, image }),
              );
              return ok ? null : "That could not be saved.";
            }}
            onRename={(id, name) =>
              void act("renamed a place", () => invoke("rename_place", { id, name }))
            }
            onResize={(id, footprint) =>
              void act("resized a place", () => invoke("resize_place", { id, footprint }))
            }
            onDelete={(id) =>
              void act("took a place down", () => invoke("remove_place", { id })).then(() =>
                editor.select({ kind: "none" }),
              )
            }
            onDisconnect={(from, to) =>
              void act("removed a road", () =>
                invoke("connect_places", { from, to, joined: false }),
              )
            }
            onTraceRoad={(road) => {
              setRoadFrom(road.from);
              setRoadTo(road.to);
              setRoadVia(road.via ?? []);
              editor.setMode("road");
              editor.select({ kind: "road", id: `${road.from}-${road.to}` });
            }}
            onPut={(id) =>
              centre &&
              void act("put a place in the World", () =>
                invoke("move_place", { id, x: centre.x, y: centre.y }),
              )
            }
            onScale={(characterId, scale) =>
              void act(scale === null ? "reset a height" : "resized somebody", () =>
                invoke("set_character_scale", { characterId, scale }),
              )
            }
            onRoutinePlace={(characterId, activity, placeId) =>
              void act(
                placeId === null ? "sent a routine home" : "moved a routine",
                () => invoke("set_routine_place", { characterId, activity, placeId }),
              )
            }
          />
        </div>
      </div>

      <StatusBar
        editor={editor}
        places={places}
        crew={characters}
        cursor={cursor}
        /*
          Zoom as a real ratio: how much of the World's height fits on screen. 100% is the
          whole World in view. The camera has no `zoom` field — it has a span in world units,
          which is the honest primitive — so this is derived rather than stored twice.
        */
        zoom={camera.camera && map ? map.height / camera.camera.span : 1}
        unreachable={unreachable}
      />

      {naming && (
        /*
          A building that has just been put down is nameless, and naming it is the next thing
          anybody wants. Asked once, here, rather than making them find the field.
        */
        <div className="wed__naming">
          <div className="pxpanel" style={{ width: 320, padding: 12 }}>
            <div className="pxtitle" style={{ marginBottom: 8 }}>
              Name this place
            </div>
            <input
              autoFocus
              className="pxinput"
              value={naming.value}
              onChange={(e) => setNaming({ ...naming, value: e.target.value })}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Escape") setNaming(null);
                if (e.key === "Enter") {
                  void act("named a place", () =>
                    invoke("rename_place", { id: naming.id, name: naming.value }),
                  ).then(() => {
                    editor.select({ kind: "place", id: naming.id });
                    setNaming(null);
                  });
                }
              }}
            />
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 6, marginTop: 12 }}>
              <button type="button" className="pxbtn" onClick={() => setNaming(null)}>
                Later
              </button>
              <button
                type="button"
                className="pxbtn pxbtn--gold"
                onClick={() =>
                  void act("named a place", () =>
                    invoke("rename_place", { id: naming.id, name: naming.value }),
                  ).then(() => {
                    editor.select({ kind: "place", id: naming.id });
                    setNaming(null);
                  })
                }
              >
                Confirm
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

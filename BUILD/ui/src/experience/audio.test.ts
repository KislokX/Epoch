/**
 * The two failures this file exists to prevent, and one that already shipped once.
 */

import { beforeEach, describe, expect, it } from "vitest";

import {
  offer,
  setVoiceVolume,
  setVoicesOn,
  voiceVolume,
  voicesOn,
} from "./audio";

const device = (
  kind: MediaDeviceKind,
  deviceId: string,
  label: string,
): MediaDeviceInfo =>
  ({ kind, deviceId, label, groupId: "", toJSON: () => ({}) }) as MediaDeviceInfo;

describe("what the browser said, turned into what a person is offered", () => {
  it("tells an unasked browser from a machine with no devices", () => {
    // Measured in Epoch's own window: before permission, three devices, every label empty.
    const unasked = offer([
      device("audioinput", "", ""),
      device("videoinput", "", ""),
      device("audiooutput", "", ""),
    ]);
    expect(unasked.asked).toBe(false);

    // Which must not read the same as a machine that genuinely has nothing.
    expect(offer([]).asked).toBe(false);
    expect(unasked.inputs).toEqual([]);
  });

  it("lists one pair of headphones once, not three times", () => {
    // Exactly what Windows answered here: `Default -`, `Communications -` and the device.
    const real = offer([
      device("audiooutput", "default", "Default - Speakers (Razer Barracuda X 2.4)"),
      device("audiooutput", "communications", "Communications - Speakers (Razer Barracuda X 2.4)"),
      device("audiooutput", "8fad908e7a", "Speakers (Razer Barracuda X 2.4)"),
      device("audiooutput", "2d8fdd7257", "SONY TV (NVIDIA High Definition Audio)"),
    ]);

    expect(real.asked).toBe(true);
    expect(real.outputs.map((it) => it.label)).toEqual([
      // Kept and renamed: following the system is a real choice and survives the hardware
      // changing under it.
      "Follow the system",
      "Speakers (Razer Barracuda X 2.4)",
      "SONY TV (NVIDIA High Definition Audio)",
    ]);
    // The alias is dropped by its reserved id, never by matching words in its label.
    expect(real.outputs.some((it) => it.id === "communications")).toBe(false);
  });

  it("offers no system entry on a platform that publishes none", () => {
    const real = offer([device("audioinput", "abc", "A microphone")]);
    expect(real.inputs.map((it) => it.label)).toEqual(["A microphone"]);
  });

  it("keeps inputs and outputs apart, and ignores cameras", () => {
    const both = offer([
      device("audioinput", "mic", "A microphone"),
      device("videoinput", "cam", "A camera"),
      device("audiooutput", "spk", "Some speakers"),
    ]);
    expect(both.inputs.map((it) => it.label)).toEqual(["A microphone"]);
    expect(both.outputs.map((it) => it.label)).toEqual(["Some speakers"]);
  });
});

describe("whether anybody speaks right now", () => {
  beforeEach(() => {
    globalThis.localStorage?.clear();
  });

  it("is on for somebody who has never touched it", () => {
    // A crew that arrives silent on a fresh machine looks broken, and a person who has never
    // seen the switch has not asked for silence.
    expect(voicesOn()).toBe(true);
  });

  it("remembers being turned off, and back on", () => {
    setVoicesOn(false);
    expect(voicesOn()).toBe(false);
    setVoicesOn(true);
    expect(voicesOn()).toBe(true);
  });

  it("is a different switch from the interface's own sound", () => {
    // One control deciding both would mean muting your own clicks to silence Mage.
    setVoicesOn(false);
    expect(globalThis.localStorage.getItem("epoch.sfx.volume")).toBeNull();
    expect(voiceVolume()).toBe(1);
  });
});

describe("how loud the crew is", () => {
  beforeEach(() => {
    globalThis.localStorage?.clear();
  });

  it("defaults to full rather than to silence", () => {
    // `Number(null)` is 0 and `Number.isFinite(0)` is true. That exact line shipped a day of
    // complete silence in `sfx.ts`, invisible to anybody who had once moved the slider.
    expect(voiceVolume()).toBe(1);
  });

  it("remembers a level, and refuses one that is not a number", () => {
    setVoiceVolume(0.4);
    expect(voiceVolume()).toBeCloseTo(0.4);

    globalThis.localStorage.setItem("epoch.voices.volume", "loud");
    expect(voiceVolume()).toBe(1);
  });

  it("clamps rather than trusting what it is handed", () => {
    setVoiceVolume(9);
    expect(voiceVolume()).toBe(1);
    setVoiceVolume(-3);
    expect(voiceVolume()).toBe(0);
  });

  it("is a different key from the interface volume", () => {
    // The whole reason this exists: muting your own clicks must not silence Mage.
    setVoiceVolume(0);
    expect(globalThis.localStorage.getItem("epoch.sfx.volume")).toBeNull();
  });
});

"""Does a local model actually read pixels, and does the extraction prompt change the answer?

Measured rather than assumed. Two questions decide whether `see_image` is worth building the way
the roadmap describes it:

1. Does Ollama accept `images: [base64]` on a chat message, and does the model answer about the
   picture rather than about its own imagination?
2. **Does the extraction prompt matter?** The roadmap's whole design point is that the reasoner
   writes it, because a hardcoded generic prompt is the published version's own biggest failure.
   If two different prompts produce the same answer, that argument is decoration.

The image is drawn here rather than taken from the vault: a fixture with known content is the
only way to tell reading from guessing.
"""

import base64
import json
import struct
import sys
import urllib.request
import zlib

HOST = "http://127.0.0.1:11434"


def png(width, height, pixel):
    """A solid-colour PNG, written by hand so the test has no dependencies."""

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    raw = b"".join(b"\x00" + bytes(pixel) * width for _ in range(height))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def ask(model, prompt, image_b64, seconds=180):
    body = json.dumps(
        {
            "model": model,
            "stream": False,
            "messages": [{"role": "user", "content": prompt, "images": [image_b64]}],
            "options": {"temperature": 0},
        }
    ).encode()
    request = urllib.request.Request(
        f"{HOST}/api/chat", body, {"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=seconds) as answer:
        return json.load(answer)


model = sys.argv[1] if len(sys.argv) > 1 else "gemma4:12b"
# Two flat colours, so "what colour is it" has one correct answer and a guess is visible.
image = base64.b64encode(png(64, 64, (16, 96, 220))).decode()

for label, prompt in [
    ("generic", "Describe this image."),
    ("specific", "What single colour fills this image? Answer with one word."),
]:
    try:
        answer = ask(model, prompt, image)
    except Exception as err:  # noqa: BLE001 - a probe reports, it does not handle
        print(f"{label:9} FAILED: {err}")
        continue
    said = answer.get("message", {}).get("content", "").strip().replace("\n", " ")
    print(f"{label:9} {said[:220]}")

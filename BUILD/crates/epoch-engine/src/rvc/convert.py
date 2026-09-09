# Turn one RVC checkpoint into ONNX. Written by Epoch, run once per voice.
#
# ## Why this file is in Epoch's repository
#
# This project keeps Python out of the runtime, and that is unchanged: after a conversion the
# whole chain is onnxruntime. What needs PyTorch is rebuilding the model's graph so it can be
# exported, and that happens once, in Epoch's own folder, and never while anybody is talking.
#
# It is Epoch's own script rather than a third party's for one reason: **Epoch should own the
# call it makes.** The model *architecture* is somebody else's (MIT, fetched beside this), and
# the export call — its opset, its dynamic axes, and `dynamo=False` — is a decision this project
# is making and must be able to change without patching a file it does not own.
#
# ## `dynamo=False`, measured
#
# torch 2.14 defaults to the dynamo exporter, which refuses the dynamic dimensions this model
# declares:
#
#     Received user-specified dim hint Dim.DYNAMIC, but tracing inferred a static shape of 100
#
# The TorchScript path exports it correctly. That is a fact about this model and this torch, so
# it is stated here with its reason rather than left as a flag somebody deletes.

import argparse
import json
import sys
from pathlib import Path

import torch

# The architecture, fetched beside this file. Epoch does not vendor a copy: a model definition
# that drifts from the checkpoints people actually download is worse than one that is fetched.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from infer.module.models import (  # noqa: E402
    SynthesizerTrnMs256NSFsid,
    SynthesizerTrnMs768NSFsid,
)


class Speaking(torch.nn.Module):
    """The generator, with the signature onnxruntime will be handed at inference."""

    def __init__(self, net_g):
        super().__init__()
        self.net_g = net_g

    def forward(self, feats, p_len, pitch, pitchf, sid):
        return self.net_g.infer(feats, p_len, pitch, pitchf, sid)[0][0, 0]


def main() -> int:
    ask = argparse.ArgumentParser()
    ask.add_argument("--checkpoint", type=Path, required=True)
    ask.add_argument("--output", type=Path, required=True)
    ask.add_argument("--opset", type=int, default=17)
    # **How long a chunk this graph will actually be correct for.**
    #
    # The export below declares its frame axis dynamic and it is not. RVC's relative attention
    # pads with `int(length) - 1` and slices its position embeddings with plain Python
    # arithmetic, so the tracer freezes both into constants; any other length answers a Reshape
    # error deep inside the encoder. Measured, at 1341 frames:
    #
    #     Input shape:{1,2,3591299}, requested shape:{1,2,1341,2679}
    #
    # So a conversion *chooses* a window and Epoch feeds it exactly that many frames. 500 is
    # five seconds, which is longer than most sentences and short enough that the attention it
    # implies is a few megabytes rather than hundreds. It is written into the sidecar, because
    # asking the graph afterwards returns the declared axis -- the value that is wrong.
    ask.add_argument("--frames", type=int, default=500)
    # 768 for RVC v2 (ContentVec), 256 for v1. Measured by Epoch from the tensor.
    ask.add_argument("--embedding", type=int, required=True, choices=[256, 768])
    args = ask.parse_args()

    # `weights_only=True` because Epoch has already read this file's pickle and refused anything
    # outside a short allowlist. Asking torch for the same guarantee costs nothing and means the
    # promise does not depend on Epoch's reader being the only thing between here and a stranger.
    kept = torch.load(args.checkpoint, map_location="cpu", weights_only=True)

    config = kept["config"]
    # **Told, not guessed.** This read `config[14]`, and the index is not stable across the
    # config layouts RVC has shipped: a v2 model came back as 256 and torch refused with a shape
    # mismatch on `enc_p.emb_phone.weight`.
    #
    # Epoch measured that width already, out of the tensor itself, before this process started.
    # Two places deciding one number is how they disagree, so only one of them decides.
    width = args.embedding
    build = SynthesizerTrnMs768NSFsid if width == 768 else SynthesizerTrnMs256NSFsid
    net_g = build(*config, is_half=False)
    net_g.load_state_dict(kept["weight"], strict=False)
    net_g.eval().float()

    frames = args.frames
    dummy = (
        torch.rand(1, frames, width),
        torch.tensor([frames], dtype=torch.int64),
        torch.zeros(1, frames, dtype=torch.int64),
        torch.rand(1, frames),
        torch.tensor([0], dtype=torch.int64),
    )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    torch.onnx.export(
        Speaking(net_g).eval(),
        dummy,
        str(args.output),
        input_names=["feats", "p_len", "pitch", "pitchf", "sid"],
        output_names=["audio"],
        dynamic_axes={
            "feats": {1: "frames"},
            "pitch": {1: "frames"},
            "pitchf": {1: "frames"},
        },
        opset_version=args.opset,
        # See the note at the top: measured, not inherited.
        dynamo=False,
    )

    # Beside the model, so what Epoch knows about it survives the process that read it.
    beside = args.output.with_suffix(".json")
    beside.write_text(
        json.dumps(
            {
                "version": kept.get("version"),
                "sr": kept.get("sr"),
                "f0": kept.get("f0"),
                "info": kept.get("info"),
                "embedding": width,
                "window": frames,
                "speakers": config[-3] if len(config) > 3 else 1,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"ok {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

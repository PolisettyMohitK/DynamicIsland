"""Train the "hey choco" wake word on top of openWakeWord's frozen front-end.

openWakeWord turns audio into 96-number speech embeddings every 80ms; a wake word is a
small classifier over the last 16 of them. This script makes that classifier:

1. Synthesises thousands of "hey choco" clips with Piper's 904-speaker LibriTTS voice,
   plus look-alike phrases ("hey coco", "chocolate") and ordinary sentences as negatives.
2. Mixes in noise, reverb and volume changes, then runs the same mel + embedding models
   the app uses, so training sees exactly what the app sees.
3. Adds ~11 hours of real speech/noise embeddings from openWakeWord as more negatives.
4. Trains on the GPU and exports `hey_choco.onnx` ([1,16,96] -> [1,1] score).

    python train_choco.py --piper en_US-libritts_r-medium.onnx \
        --models <folder with melspectrogram.onnx and embedding_model.onnx> \
        --negatives validation_set_features.npy --out hey_choco.onnx

Needs: torch, onnx, piper-tts, onnxruntime, numpy, scipy.
"""

import argparse
import random
import time
from pathlib import Path

import numpy as np
import onnxruntime as ort
import torch
from piper import PiperVoice, SynthesisConfig
from scipy.signal import fftconvolve, resample_poly

RATE = 16_000
WINDOW = int(2.4 * RATE)  # enough audio for 16 embeddings

POSITIVE = ["hey choco", "hey choco.", "hey, choco", "hey choco!", "hey chocko", "hey cho-co"]
# Close but wrong: these must not wake it.
LOOKALIKE = [
    "hey coco", "hey cocoa", "hey jojo", "hey taco", "hey yoko", "hey poco", "hey chico", "hey chuck",
    "hey chloe", "hey joe", "hey josh", "hey joker", "hey choker", "hey jocko", "choco", "chocolate",
    "hot chocolate", "a choco bar", "hey go", "okay go", "hey Google", "hey Siri", "hey Jarvis",
    "Alexa", "hey there", "hey you", "hey Coco Chanel", "check out", "show code", "hey, so go",
    "hey Colorado", "hey Chicago", "hey cool", "hey call", "say cheese", "hey chips",
]
WORDS = (
    "the a to of and in is it you that he was for on are with as I his they be at one have this from "
    "or had by word but what some we can out other were all there when up use your how said an each "
    "she which do their time if will way about many then them write would like so these her long make "
    "thing see him two has look more day could go come did number sound no most people my over know "
    "water than call first who may down side been now find any new work part take get place made live "
    "where after back little only round man year came show every good me give our under name very "
    "through just form sentence great think say help low line differ turn cause much mean before move "
    "right boy old too same tell does set three want air well also play small end put home read hand "
    "port large spell add even land here must big high such follow act why ask men change went light "
    "kind off need house picture try us again animal point mother world near build self earth father "
    "music focus timer coffee chocolate check choose chosen coast cocoa okay hello morning tomorrow"
).split()


def sentence(rng):
    return " ".join(rng.choice(WORDS) for _ in range(rng.randint(3, 12)))


def synth(voice, text, rng):
    cfg = SynthesisConfig(
        speaker_id=rng.randrange(voice.config.num_speakers),
        length_scale=rng.uniform(0.75, 1.35),
        noise_scale=rng.uniform(0.4, 0.9),
        noise_w_scale=rng.uniform(0.5, 1.0),
    )
    audio = np.concatenate([c.audio_float_array for c in voice.synthesize(text, syn_config=cfg)])
    audio = resample_poly(audio, 320, 441)  # 22050 -> 16000
    # Trim the silence Piper leaves around the words.
    loud = np.flatnonzero(np.abs(audio) > 0.02)
    return audio[loud[0] : loud[-1] + 1] if len(loud) else audio


def coloured_noise(n, rng):
    white = np.random.default_rng(rng.randrange(1 << 30)).standard_normal(n)
    kind = rng.choice(["white", "pink", "brown"])
    if kind == "white":
        return white
    spec = np.fft.rfft(white)
    f = np.arange(len(spec)) + 1.0
    spec /= f ** (0.5 if kind == "pink" else 1.0)
    out = np.fft.irfft(spec, n)
    return out / (np.std(out) + 1e-9)


def reverb(x, rng):
    rt60 = rng.uniform(0.15, 0.8)
    n = int(rt60 * RATE)
    t = np.arange(n) / RATE
    rir = np.random.default_rng(rng.randrange(1 << 30)).standard_normal(n) * np.exp(-6.9 * t / rt60)
    rir[0] = 1.0
    y = fftconvolve(x, rir)[: len(x)]
    return y / (np.max(np.abs(y)) + 1e-9) * np.max(np.abs(x))


def place(clip, rng, background, end_gap):
    """Put `clip` in a WINDOW-long buffer so it ends `end_gap` seconds before the end, over noise."""
    out = np.zeros(WINDOW)
    end = WINDOW - int(end_gap * RATE)
    clip = clip[-end:]  # long sentences: keep the part that ends at `end`
    out[end - len(clip) : end] = clip
    if rng.random() < 0.5:
        out = reverb(out, rng)
    speech_rms = np.sqrt(np.mean(clip**2)) + 1e-9
    snr = rng.uniform(3, 30)
    if rng.random() < 0.35 and background:
        noise = rng.choice(background)
        noise = np.resize(noise, WINDOW) / (np.sqrt(np.mean(noise**2)) + 1e-9)
    else:
        noise = coloured_noise(WINDOW, rng)
    out = out + noise * speech_rms / (10 ** (snr / 20))
    # The app gets int16-scale samples from quiet to loud microphones.
    return out / (np.max(np.abs(out)) + 1e-9) * rng.uniform(1500, 30000)


class FrontEnd:
    """The app's openWakeWord front-end: audio -> mel frames -> 96-d embeddings."""

    def __init__(self, models):
        opts = ort.SessionOptions()
        providers = ["CPUExecutionProvider"]
        self.mel = ort.InferenceSession(str(models / "melspectrogram.onnx"), opts, providers=providers)
        self.emb = ort.InferenceSession(str(models / "embedding_model.onnx"), opts, providers=providers)

    def last16(self, audio, extra=0, rng=None):
        """Embeddings for the 16 steps ending at the end of `audio` (plus `extra` earlier windows)."""
        mel = self.mel.run(None, {"input": audio[None].astype(np.float32)})[0].reshape(-1, 32) / 10 + 2
        starts = list(range(len(mel) - 76, -1, -8))[::-1]  # embedding windows, 8 frames apart
        windows = np.stack([mel[s : s + 76] for s in starts])[..., None].astype(np.float32)
        emb = self.emb.run(None, {"input_1": windows})[0].reshape(len(starts), 96)
        out = [emb[-16:]]
        for _ in range(extra):
            end = rng.randrange(16, len(emb))
            out.append(emb[end - 16 : end])
        return out


class Net(torch.nn.Module):
    def __init__(self):
        super().__init__()
        self.body = torch.nn.Sequential(
            torch.nn.Flatten(),
            torch.nn.Linear(16 * 96, 128),
            torch.nn.LayerNorm(128),
            torch.nn.ReLU(),
            torch.nn.Linear(128, 128),
            torch.nn.LayerNorm(128),
            torch.nn.ReLU(),
            torch.nn.Linear(128, 1),
        )

    def forward(self, x):
        return torch.sigmoid(self.body(x))


def build(args, rng, fe):
    voice = PiperVoice.load(args.piper)
    t = time.time()
    lookalike = [synth(voice, rng.choice(LOOKALIKE), rng) for _ in range(args.n_neg // 2)]
    chatter = [synth(voice, sentence(rng), rng) for _ in range(args.n_neg // 2)]
    print(f"negatives synthesised in {time.time() - t:.0f}s", flush=True)
    background = chatter[: len(chatter) // 3]

    pos, neg = [], []
    t = time.time()
    for i in range(args.n_pos):
        clip = synth(voice, rng.choice(POSITIVE), rng)
        for _ in range(2):
            pos += fe.last16(place(clip, rng, background, rng.uniform(0.0, 0.3)))
        if i % 500 == 0:
            print(f"positives {i}/{args.n_pos} ({time.time() - t:.0f}s)", flush=True)
    for clip in lookalike:
        neg += fe.last16(place(clip, rng, background, rng.uniform(0.0, 0.3)))
        # The start of "hey choco" alone ("hey cho...") shouldn't fire either.
    for clip in chatter:
        neg += fe.last16(place(clip, rng, background, rng.uniform(0.0, 0.5)), extra=2, rng=rng)
    for _ in range(args.n_neg // 4):
        neg += fe.last16(place(np.zeros(1), rng, background, 0.0))  # pure noise
    # Half of each positive clip: the phrase cut off early is not the wake word.
    for _ in range(args.n_neg // 4):
        clip = synth(voice, rng.choice(POSITIVE), rng)
        neg += fe.last16(place(clip[: int(len(clip) * rng.uniform(0.3, 0.6))], rng, background, 0.0))
    return np.array(pos, np.float32), np.array(neg, np.float32)


def windows(flat, stride):
    idx = np.arange(0, len(flat) - 16, stride)
    return np.stack([flat[i : i + 16] for i in idx]).astype(np.float32)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--piper", required=True)
    p.add_argument("--models", required=True, type=Path)
    p.add_argument("--negatives", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--n-pos", type=int, default=5000)
    p.add_argument("--n-neg", type=int, default=6000)
    p.add_argument("--epochs", type=int, default=60)
    p.add_argument("--cache", default="choco_features.npz")
    args = p.parse_args()
    rng = random.Random(7)

    if Path(args.cache).exists():
        d = np.load(args.cache)
        pos, neg = d["pos"], d["neg"]
    else:
        pos, neg = build(args, rng, FrontEnd(args.models))
        np.savez(args.cache, pos=pos, neg=neg)
    print("positives", pos.shape, "negatives", neg.shape, flush=True)

    # Real-world audio: 80% to train on, the rest to count false wakes per hour.
    flat = np.load(args.negatives)
    cut = int(len(flat) * 0.8)
    real_train, real_test = windows(flat[:cut], 4), windows(flat[cut:], 1)
    hours_test = len(real_test) * 0.08 / 3600

    split = int(len(pos) * 0.9)
    x = np.concatenate([pos[:split], neg, real_train])
    y = np.concatenate([np.ones(split), np.zeros(len(neg) + len(real_train))]).astype(np.float32)
    dev = "cuda" if torch.cuda.is_available() else "cpu"
    X, Y = torch.tensor(x, device=dev), torch.tensor(y, device=dev)
    # Missing the wake word is annoying; waking up by itself is worse. Weight negatives up.
    w = torch.where(Y > 0, 1.0, 3.0 * split / (len(y) - split))

    net = Net().to(dev)
    opt = torch.optim.AdamW(net.parameters(), lr=1e-3, weight_decay=1e-2)
    sched = torch.optim.lr_scheduler.CosineAnnealingLR(opt, args.epochs)
    for epoch in range(args.epochs):
        net.train()
        perm = torch.randperm(len(X), device=dev)
        for i in range(0, len(X), 1024):
            b = perm[i : i + 1024]
            loss = torch.nn.functional.binary_cross_entropy(net(X[b]).squeeze(1), Y[b], weight=w[b])
            opt.zero_grad()
            loss.backward()
            opt.step()
        sched.step()
        if epoch % 10 == 9 or epoch == args.epochs - 1:
            net.eval()
            with torch.no_grad():
                recall = (net(torch.tensor(pos[split:], device=dev)) > 0.5).float().mean().item()
                scores = torch.cat(
                    [net(torch.tensor(real_test[i : i + 8192], device=dev)) for i in range(0, len(real_test), 8192)]
                ).squeeze(1).cpu().numpy()
            # A wake is a run of high scores; count runs, not frames.
            hits = scores > 0.5
            wakes = int(np.sum(hits[1:] & ~hits[:-1]) + hits[0])
            print(f"epoch {epoch + 1}: loss {loss.item():.4f}  recall {recall:.3f}  "
                  f"false wakes {wakes / hours_test:.2f}/hour", flush=True)

    net.eval().cpu()
    torch.onnx.export(net, torch.zeros(1, 16, 96), args.out, input_names=["x"], output_names=["score"],
                      dynamic_axes={"x": {0: "batch"}, "score": {0: "batch"}}, opset_version=17, dynamo=False)
    print("saved", args.out)


if __name__ == "__main__":
    main()

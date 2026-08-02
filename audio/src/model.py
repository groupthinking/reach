"""Audio Flamingo model loader with multi-backend support.

Supported backends (MODEL_BACKEND env var):
  af3   — Audio Flamingo 3 (7B, sound+music+speech, up to 10min audio)
  af2   — Audio Flamingo 2 (3B, long-audio up to 5min)
  music — Music Flamingo (AF3 backbone, music/song understanding)
"""
from __future__ import annotations

import os
import logging
from typing import Any, Tuple

import torch
from transformers import AutoTokenizer, AutoModelForCausalLM, AutoProcessor

logger = logging.getLogger(__name__)

MODEL_CONFIGS: dict[str, dict[str, str]] = {
    "af3": {
        "hf_repo": "nvidia/audio-flamingo-3-hf",
        "description": "Audio Flamingo 3 — 7B, sound+music+speech, up to 10min audio",
    },
    "af2": {
        "hf_repo": "nvidia/audio-flamingo-2",
        "description": "Audio Flamingo 2 — 3B, long-audio up to 5min",
    },
    "music": {
        "hf_repo": "nvidia/music-flamingo-hf",
        "description": "Music Flamingo — AF3 backbone, music/song understanding",
    },
}

_loaded_model: Any = None
_loaded_tokenizer: Any = None
_loaded_backend: str | None = None


def load_model(backend: str) -> Tuple[Any, Any]:
    """Load tokenizer and model for the given backend.
    Caches the loaded model globally; reloads if backend changes.
    """
    global _loaded_model, _loaded_tokenizer, _loaded_backend

    if _loaded_backend == backend and _loaded_model is not None:
        return _loaded_model, _loaded_tokenizer

    if backend not in MODEL_CONFIGS:
        raise ValueError(
            f"Unknown MODEL_BACKEND '{backend}'. "
            f"Valid options: {list(MODEL_CONFIGS.keys())}"
        )

    cfg = MODEL_CONFIGS[backend]
    hf_repo = cfg["hf_repo"]
    hf_token = os.getenv("HF_TOKEN") or None

    logger.info("Loading model backend=%s from %s", backend, hf_repo)

    device = "cuda" if torch.cuda.is_available() else "cpu"
    logger.info("Using device: %s", device)

    tokenizer = AutoProcessor.from_pretrained(
        hf_repo,
        token=hf_token,
        trust_remote_code=True,
    )
    model = AutoModelForCausalLM.from_pretrained(
        hf_repo,
        token=hf_token,
        torch_dtype=torch.float16 if device == "cuda" else torch.float32,
        device_map="auto" if device == "cuda" else None,
        trust_remote_code=True,
    )
    if device == "cpu":
        model = model.to(device)
    model.eval()

    _loaded_model = model
    _loaded_tokenizer = tokenizer
    _loaded_backend = backend

    logger.info("Model loaded: %s", cfg["description"])
    return model, tokenizer


def run_inference(audio_input: Any, model: Any, tokenizer: Any) -> str:
    """Run forward pass on audio_input and return text output.

    audio_input: numpy array (float32, mono, 16kHz) or path string.
    Returns the model's text response.
    """
    import numpy as np

    prompt = "Describe this audio in detail, including any speech transcript, "\
             "sound events, music, and overall understanding."

    try:
        inputs = tokenizer(
            text=prompt,
            audio=audio_input,
            return_tensors="pt",
            sampling_rate=16000,
        )
    except TypeError:
        # Fallback: some processor variants use different kwargs
        inputs = tokenizer(
            prompt,
            return_tensors="pt",
        )

    device = next(model.parameters()).device
    inputs = {k: v.to(device) for k, v in inputs.items()}

    with torch.no_grad():
        output_ids = model.generate(
            **inputs,
            max_new_tokens=512,
            do_sample=False,
        )

    output_text = tokenizer.decode(output_ids[0], skip_special_tokens=True)
    # Strip the prompt from the output if echoed
    if output_text.startswith(prompt):
        output_text = output_text[len(prompt):].strip()

    return output_text

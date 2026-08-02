"""Audio Flamingo inference service — FastAPI app."""
from __future__ import annotations

import io
import logging
import os
import tempfile
from contextlib import asynccontextmanager
from typing import Optional

import httpx
import librosa
import numpy as np
import soundfile as sf
from dotenv import load_dotenv
from fastapi import FastAPI, File, Form, HTTPException, UploadFile
from fastapi.responses import JSONResponse
from pydantic import BaseModel

from .model import MODEL_CONFIGS, load_model, run_inference

load_dotenv()

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

BACKEND = os.getenv("MODEL_BACKEND", "af3")
_model = None
_tokenizer = None
_model_loaded = False
_model_error: Optional[str] = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    global _model, _tokenizer, _model_loaded, _model_error
    try:
        _model, _tokenizer = load_model(BACKEND)
        _model_loaded = True
        logger.info("Model ready: backend=%s", BACKEND)
    except Exception as exc:
        _model_error = str(exc)
        logger.error("Failed to load model: %s", exc)
    yield


app = FastAPI(title="Reach Audio Flamingo Service", lifespan=lifespan)


class WebhookPayload(BaseModel):
    event_id: str
    audio_url: str
    community_id: str
    channel_id: str


class AudioUrlRequest(BaseModel):
    audio_url: str


def _load_audio_from_bytes(data: bytes) -> np.ndarray:
    """Load audio bytes → float32 numpy array at 16kHz mono."""
    with io.BytesIO(data) as buf:
        audio, sr = sf.read(buf)
    if audio.ndim > 1:
        audio = audio.mean(axis=1)
    if sr != 16000:
        audio = librosa.resample(audio.astype(np.float32), orig_sr=sr, target_sr=16000)
    return audio.astype(np.float32)


async def _load_audio_from_url(url: str) -> np.ndarray:
    async with httpx.AsyncClient(timeout=60) as client:
        resp = await client.get(url)
        resp.raise_for_status()
    return _load_audio_from_bytes(resp.content)


@app.get("/health")
async def health():
    return {
        "status": "ok" if _model_loaded else "loading",
        "model_loaded": _model_loaded,
        "backend": BACKEND,
        "description": MODEL_CONFIGS.get(BACKEND, {}).get("description", ""),
        "error": _model_error,
    }


@app.post("/infer")
async def infer(
    audio: Optional[UploadFile] = File(None),
    audio_url: Optional[str] = Form(None),
):
    if not _model_loaded:
        raise HTTPException(status_code=503, detail="Model not loaded")

    if audio is not None:
        raw = await audio.read()
        audio_array = _load_audio_from_bytes(raw)
    elif audio_url:
        audio_array = await _load_audio_from_url(audio_url)
    else:
        raise HTTPException(
            status_code=422,
            detail="Provide either 'audio' file or 'audio_url'",
        )

    output = run_inference(audio_array, _model, _tokenizer)

    # Heuristic: first sentence is transcript, rest is understanding
    parts = output.split(". ", 1)
    transcript = parts[0] if parts else output
    understanding = parts[1] if len(parts) > 1 else ""

    return {
        "transcript": transcript,
        "understanding": understanding,
        "model": BACKEND,
    }


@app.post("/webhook")
async def webhook(payload: WebhookPayload):
    """Receive relay audio event, run inference, post result back to relay channel."""
    if not _model_loaded:
        raise HTTPException(status_code=503, detail="Model not loaded")

    audio_array = await _load_audio_from_url(payload.audio_url)
    output = run_inference(audio_array, _model, _tokenizer)

    parts = output.split(". ", 1)
    transcript = parts[0] if parts else output
    understanding = parts[1] if len(parts) > 1 else ""

    relay_url = os.getenv("RELAY_WEBHOOK_URL")
    if relay_url:
        result_payload = {
            "event_id": payload.event_id,
            "community_id": payload.community_id,
            "channel_id": payload.channel_id,
            "transcript": transcript,
            "understanding": understanding,
            "model": BACKEND,
        }
        async with httpx.AsyncClient(timeout=30) as client:
            try:
                await client.post(relay_url, json=result_payload)
            except Exception as exc:
                logger.warning("Failed to post result to relay: %s", exc)

    return {
        "transcript": transcript,
        "understanding": understanding,
        "model": BACKEND,
    }

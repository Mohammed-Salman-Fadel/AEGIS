import os
import logging
import tempfile
from faster_whisper import WhisperModel
from kokoro_onnx import Kokoro
import soundfile as sf

logger = logging.getLogger(__name__)

class VoiceService:
    def __init__(self):
        self.stt_model = None
        self.tts_model = None
        self.model_size = "tiny" # Best for speed/CPU
        
        # Paths for Kokoro
        # Note: We look in rag-python/models/kokoro
        base_path = os.path.join(os.getcwd(), "models", "kokoro")
        self.kokoro_model_path = os.path.join(base_path, "kokoro-v0_19.onnx")
        self.kokoro_voices_path = os.path.join(base_path, "voices.json")
        
    def load_stt(self):
        """Lazy load the Whisper model"""
        if self.stt_model is None:
            logger.info(f"Loading Whisper model: {self.model_size}")
            try:
                # Use CPU by default for maximum compatibility
                self.stt_model = WhisperModel(
                    self.model_size, 
                    device="cpu", 
                    compute_type="int8",
                    download_root=os.path.join(os.getcwd(), "models", "whisper")
                )
            except Exception as e:
                logger.error(f"Failed to load Whisper model: {e}")
                raise e
            
    def load_tts(self):
        """Lazy load the Kokoro model"""
        if self.tts_model is None:
            if not os.path.exists(self.kokoro_model_path):
                logger.warning(f"Kokoro model not found at {self.kokoro_model_path}. TTS disabled.")
                return False
            
            logger.info("Loading Kokoro TTS model...")
            try:
                self.tts_model = Kokoro(self.kokoro_model_path, self.kokoro_voices_path)
                return True
            except Exception as e:
                logger.error(f"Failed to load Kokoro: {e}")
                return False
        return True

    def transcribe(self, audio_bytes: bytes):
        """Transcribes audio bytes to text"""
        self.load_stt()
        
        with tempfile.NamedTemporaryFile(delete=False, suffix=".wav") as temp_audio:
            temp_audio.write(audio_bytes)
            temp_path = temp_audio.name
            
        try:
            segments, info = self.stt_model.transcribe(temp_path, beam_size=5)
            text = " ".join([segment.text for segment in segments])
            return text.strip()
        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)

    def synthesize(self, text: str):
        """Synthesizes text to speech using Kokoro and returns the WAV bytes"""
        if not self.load_tts():
            return None
            
        try:
            import io
            # Generate audio samples (af_bella is a high-quality female voice)
            samples, sample_rate = self.tts_model.create(text, voice="af_bella", speed=1.0)
            
            # Save to an in-memory BytesIO buffer
            buffer = io.BytesIO()
            sf.write(buffer, samples, sample_rate, format='WAV', subtype='PCM_16')
            return buffer.getvalue()
        except Exception as e:
            logger.error(f"Synthesis failed: {e}")
            return None

voice_service = VoiceService()

import os
import logging
import tempfile

try:
    from faster_whisper import WhisperModel
except ImportError:  # Optional dependency: speech-to-text may be unavailable.
    WhisperModel = None

try:
    from kokoro_onnx import Kokoro
except ImportError:  # Optional dependency: text-to-speech may be unavailable.
    Kokoro = None

try:
    import soundfile as sf
except ImportError:  # Optional dependency: TTS output encoding may be unavailable.
    sf = None

logger = logging.getLogger(__name__)

class VoiceService:
    def __init__(self):
        self.stt_model = None
        self.tts_model = None
        self.model_size = "tiny" # Best for speed/CPU
        self.keep_cached = True  # If False, models are unloaded after each query
        
        # Paths for Kokoro
        # Note: We look in python-services/models/kokoro
        base_path = os.path.join(os.getcwd(), "models", "kokoro")
        self.kokoro_model_path = os.path.join(base_path, "kokoro-v0_19.onnx")
        self.kokoro_voices_path = os.path.join(base_path, "voices.json")
        
    def load_stt(self):
        """Lazy load the Whisper model"""
        if WhisperModel is None:
            logger.warning("faster_whisper is not installed. Speech-to-text is disabled.")
            return False
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
        if Kokoro is None:
            logger.warning("kokoro_onnx is not installed. Text-to-speech is disabled.")
            return False
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

    def unload_models(self):
        """Unloads Whisper and Kokoro models from memory to free RAM"""
        import gc
        logger.info("Unloading voice models from memory to free RAM...")
        self.stt_model = None
        self.tts_model = None
        gc.collect()
 
    def transcribe(self, audio_bytes: bytes):
        """Transcribes audio bytes to text"""
        if not self.load_stt():
            return ""
        
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
            if not self.keep_cached:
                self.unload_models()
 
    def synthesize(self, text: str):
        """Synthesizes text to speech using Kokoro and returns the WAV bytes"""
        if sf is None:
            logger.warning("soundfile is not installed. Text-to-speech is disabled.")
            return None
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
        finally:
            if not self.keep_cached:
                self.unload_models()
 
voice_service = VoiceService()

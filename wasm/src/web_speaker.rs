use emu86_core::SpeakerOutput;
use wasm_bindgen::JsValue;
use web_sys::{AudioContext, GainNode, OscillatorNode, OscillatorType};

/// Web Audio API-based PC speaker implementation
pub struct WebSpeaker {
    audio_context: AudioContext,
    gain_node: GainNode,
    oscillator: Option<OscillatorNode>,
    current_frequency: f32,
    is_enabled: bool,
    /// Cached frequency to avoid unnecessary oscillator recreation
    last_frequency: f32,
}

impl WebSpeaker {
    /// Create a new WebSpeaker with Web Audio API
    ///
    /// Returns an error if Web Audio API is unavailable or initialization fails.
    /// Note: Some browsers require user interaction before audio can play (autoplay policy).
    pub fn new() -> Result<Self, JsValue> {
        let audio_context = AudioContext::new()?;

        // Create gain node for volume control
        let gain_node = audio_context.create_gain()?;
        gain_node.connect_with_audio_node(&audio_context.destination())?;

        // Start with gain at 0 (muted)
        gain_node.gain().set_value(0.0);

        log::info!(
            "Web Audio API initialized: sample_rate={} Hz",
            audio_context.sample_rate()
        );

        Ok(Self {
            audio_context,
            gain_node,
            oscillator: None,
            current_frequency: 0.0,
            is_enabled: false,
            last_frequency: 0.0,
        })
    }

    /// Create and start a new oscillator at the specified frequency
    fn start_oscillator(&mut self, frequency: f32) -> Result<(), JsValue> {
        // Stop any existing oscillator
        self.stop_oscillator();

        // Create new oscillator
        let oscillator = self.audio_context.create_oscillator()?;
        oscillator.set_type(OscillatorType::Square);
        oscillator.frequency().set_value(frequency);
        oscillator.connect_with_audio_node(&self.gain_node)?;

        // Start immediately
        oscillator.start()?;

        self.oscillator = Some(oscillator);
        self.current_frequency = frequency;

        Ok(())
    }

    /// Stop the current oscillator
    fn stop_oscillator(&mut self) {
        if let Some(osc) = self.oscillator.take() {
            // Ignore errors when stopping (oscillator might already be stopped)
            let _ = osc.stop();
            let _ = osc.disconnect();
        }
        self.current_frequency = 0.0;
    }
}

// WebSpeaker is safe to send between threads in WASM because WASM is single-threaded.
// The web_sys types are not Send because they contain raw pointers to JavaScript objects,
// but in practice there are no threads in WASM that could cause issues.
unsafe impl Send for WebSpeaker {}

impl SpeakerOutput for WebSpeaker {
    fn set_frequency(&mut self, enabled: bool, frequency: f32) {
        // Handle enable/disable state change
        if enabled != self.is_enabled {
            if enabled {
                log::debug!("WebSpeaker: Enabling at {:.2} Hz", frequency);
                // Set volume to 30% (matches RodioSpeaker)
                self.gain_node.gain().set_value(0.3);
            } else {
                log::debug!("WebSpeaker: Disabling");
                // Mute
                self.gain_node.gain().set_value(0.0);
                // Stop oscillator to save CPU
                self.stop_oscillator();
            }
            self.is_enabled = enabled;
        }

        if !enabled {
            return; // Don't update frequency when disabled
        }

        // Only update frequency if it changed significantly (> 1 Hz)
        // This avoids unnecessary oscillator recreation
        if (frequency - self.last_frequency).abs() > 1.0 {
            if let Err(e) = self.start_oscillator(frequency) {
                log::warn!("Failed to start oscillator: {:?}", e);
            }
            self.last_frequency = frequency;
        }
    }

    fn update(&mut self) {
        // Web Audio API handles buffering automatically - no action needed
    }
}

impl Drop for WebSpeaker {
    fn drop(&mut self) {
        // Clean up: stop oscillator and close audio context
        self.stop_oscillator();
        if let Err(e) = self.audio_context.close() {
            log::warn!("Failed to close AudioContext: {:?}", e);
        }
    }
}

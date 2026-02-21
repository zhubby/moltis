//! Voicebox TTS (local voice-cloning) provider.
//!
//! Voicebox is a local Qwen3-TTS server with a FastAPI REST API for voice
//! cloning. Generation is two-step: POST /generate returns metadata with a
//! generation ID, then GET /audio/{id} fetches the WAV bytes.
//!
//! Run the server: voicebox serve (default port 8000)

use {
    crate::{
        config::VoiceboxTtsConfig,
        tts::{AudioFormat, AudioOutput, SynthesizeRequest, TtsProvider, Voice},
    },
    anyhow::{Result, anyhow},
    async_trait::async_trait,
    bytes::Bytes,
    reqwest::Client,
    serde::{Deserialize, Serialize},
};

const DEFAULT_ENDPOINT: &str = "http://localhost:8000";

/// Voicebox TTS (local voice-cloning server) provider.
pub struct VoiceboxTts {
    endpoint: String,
    profile_id: Option<String>,
    model_size: Option<String>,
    language: Option<String>,
    client: Client,
}

impl VoiceboxTts {
    /// Create a new Voicebox TTS provider from config.
    #[must_use]
    pub fn new(config: &VoiceboxTtsConfig) -> Self {
        Self {
            endpoint: config.endpoint.clone(),
            profile_id: config.profile_id.clone(),
            model_size: config.model_size.clone(),
            language: config.language.clone(),
            client: Client::new(),
        }
    }
}

/// Request body for POST /generate.
#[derive(Serialize)]
struct GenerateRequest<'a> {
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_size: Option<&'a str>,
}

/// Response from POST /generate.
#[derive(Deserialize)]
struct GenerateResponse {
    generation_id: String,
}

/// A voice profile from GET /profiles.
#[derive(Deserialize)]
struct ProfileEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[async_trait]
impl TtsProvider for VoiceboxTts {
    fn id(&self) -> &'static str {
        "voicebox"
    }

    fn name(&self) -> &'static str {
        "Voicebox"
    }

    fn is_configured(&self) -> bool {
        // Configured if endpoint differs from default or a profile is set
        self.endpoint != DEFAULT_ENDPOINT || self.profile_id.is_some()
    }

    async fn voices(&self) -> Result<Vec<Voice>> {
        let url = format!("{}/profiles", self.endpoint);

        let resp = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(profiles) = r.json::<Vec<ProfileEntry>>().await {
                    let voices = profiles
                        .into_iter()
                        .map(|p| Voice {
                            id: p.id.clone(),
                            name: p.name.unwrap_or(p.id),
                            description: p.description,
                            preview_url: None,
                        })
                        .collect();
                    return Ok(voices);
                }
            },
            _ => {},
        }

        // Fallback when server is unreachable or returns unexpected data
        Ok(vec![Voice {
            id: "default".into(),
            name: "Default".into(),
            description: Some("Voicebox default voice".into()),
            preview_url: None,
        }])
    }

    async fn synthesize(&self, request: SynthesizeRequest) -> Result<AudioOutput> {
        // Step 1: POST /generate to create the generation
        let profile_id = request.voice_id.as_deref().or(self.profile_id.as_deref());

        let body = GenerateRequest {
            text: &request.text,
            profile_id,
            language: self.language.as_deref(),
            seed: None,
            model_size: self.model_size.as_deref(),
        };

        let generate_url = format!("{}/generate", self.endpoint);
        let resp = self
            .client
            .post(&generate_url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to connect to Voicebox server at {}: {}. Start with: voicebox serve",
                    self.endpoint,
                    e
                )
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Voicebox generate error {}: {}", status, body));
        }

        let generation: GenerateResponse = resp
            .json()
            .await
            .map_err(|e| anyhow!("Voicebox: invalid generate response: {}", e))?;

        // Step 2: GET /audio/{generation_id} to fetch WAV bytes
        let audio_url = format!("{}/audio/{}", self.endpoint, generation.generation_id);
        let audio_resp = self
            .client
            .get(&audio_url)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| anyhow!("Voicebox: failed to fetch audio: {}", e))?;

        if !audio_resp.status().is_success() {
            let status = audio_resp.status();
            let body = audio_resp.text().await.unwrap_or_default();
            return Err(anyhow!("Voicebox audio fetch error {}: {}", status, body));
        }

        let audio_data = audio_resp.bytes().await?;

        Ok(AudioOutput {
            data: Bytes::from(audio_data.to_vec()),
            format: AudioFormat::Pcm,
            duration_ms: None,
        })
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voicebox_not_configured_with_defaults() {
        let config = VoiceboxTtsConfig::default();
        let tts = VoiceboxTts::new(&config);
        assert!(!tts.is_configured());
    }

    #[test]
    fn test_voicebox_configured_with_profile() {
        let config = VoiceboxTtsConfig {
            profile_id: Some("abc-123-def".into()),
            ..Default::default()
        };
        let tts = VoiceboxTts::new(&config);
        assert!(tts.is_configured());
    }

    #[test]
    fn test_voicebox_configured_with_custom_endpoint() {
        let config = VoiceboxTtsConfig {
            endpoint: "http://192.168.1.100:8000".into(),
            ..Default::default()
        };
        let tts = VoiceboxTts::new(&config);
        assert!(tts.is_configured());
    }

    #[test]
    fn test_voicebox_id_and_name() {
        let config = VoiceboxTtsConfig::default();
        let tts = VoiceboxTts::new(&config);
        assert_eq!(tts.id(), "voicebox");
        assert_eq!(tts.name(), "Voicebox");
        assert!(!tts.supports_ssml());
    }

    #[test]
    fn test_generate_request_serialization() {
        let req = GenerateRequest {
            text: "Hello world",
            profile_id: Some("abc-123"),
            language: Some("en"),
            seed: None,
            model_size: Some("1.7B"),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["text"], "Hello world");
        assert_eq!(json["profile_id"], "abc-123");
        assert_eq!(json["language"], "en");
        assert!(json.get("seed").is_none());
        assert_eq!(json["model_size"], "1.7B");
    }

    #[test]
    fn test_generate_request_minimal_serialization() {
        let req = GenerateRequest {
            text: "Hello",
            profile_id: None,
            language: None,
            seed: None,
            model_size: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["text"], "Hello");
        // Optional fields should be absent
        assert!(json.get("profile_id").is_none());
        assert!(json.get("language").is_none());
        assert!(json.get("seed").is_none());
        assert!(json.get("model_size").is_none());
    }
}

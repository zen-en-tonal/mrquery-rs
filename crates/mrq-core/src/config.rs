use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrqConfig {
    #[serde(default)]
    pub image: ImageConfig,
    #[serde(default)]
    pub wavelet: WaveletConfig,
    #[serde(default)]
    pub color: ColorConfig,
    #[serde(default)]
    pub edge: EdgeConfig,
    #[serde(default)]
    pub hash: HashConfig,
    #[serde(default)]
    pub scoring: ScoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub size: u32,
    pub max_input_pixels: u64,
    pub background: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveletConfig {
    pub kind: String,
    pub top_k: usize,
    pub channels: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorConfig {
    pub hist_bins: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeConfig {
    pub method: String,
    pub orientation_bins: usize,
    pub grid: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashConfig {
    pub kind: String,
    pub bits: u32,
    pub prefix_bits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    pub image: ScoringWeights,
    pub sketch: ScoringWeights,
    pub duplicate: ScoringWeights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    // All fields already pub below
    pub wavelet: f32,
    pub color: f32,
    pub edge: f32,
    pub hash: f32,
    pub aspect: f32,
}

impl Default for MrqConfig {
    fn default() -> Self {
        Self {
            image: ImageConfig::default(),
            wavelet: WaveletConfig::default(),
            color: ColorConfig::default(),
            edge: EdgeConfig::default(),
            hash: HashConfig::default(),
            scoring: ScoringConfig::default(),
        }
    }
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            size: 128,
            max_input_pixels: 40_000_000,
            background: [1.0, 1.0, 1.0],
        }
    }
}

impl Default for WaveletConfig {
    fn default() -> Self {
        Self {
            kind: "haar".to_string(),
            top_k: 64,
            channels: "rgb".to_string(),
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self { hist_bins: 8 }
    }
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            method: "sobel".to_string(),
            orientation_bins: 8,
            grid: 4,
        }
    }
}

impl Default for HashConfig {
    fn default() -> Self {
        Self {
            kind: "average".to_string(),
            bits: 64,
            prefix_bits: 16,
        }
    }
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            image: ScoringWeights {
                wavelet: 1.00,
                color: 0.25,
                edge: 0.50,
                hash: 0.15,
                aspect: 0.10,
            },
            sketch: ScoringWeights {
                wavelet: 0.60,
                color: 0.05,
                edge: 1.00,
                hash: 0.05,
                aspect: 0.20,
            },
            duplicate: ScoringWeights {
                wavelet: 0.40,
                color: 0.30,
                edge: 0.10,
                hash: 1.00,
                aspect: 0.30,
            },
        }
    }
}

impl MrqConfig {
    pub fn from_toml(s: &str) -> crate::Result<Self> {
        toml::from_str(s).map_err(|e| crate::MrqError::Config(e.to_string()))
    }
}

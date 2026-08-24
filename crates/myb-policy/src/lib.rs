use myb_core::{PolicyEngine as PolicyEngineTrait, VolumeDecision};
use serde::{Deserialize, Serialize};

/// A keyword match configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordMatch {
    pub keywords: Vec<String>,
    pub threshold: f64,
}

/// An action to execute when a policy matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    /// Volume level in the range [0, 100].
    pub volume: u32,
    /// How long to keep the volume after the last hit, in seconds.
    pub duration_seconds: u32,
    /// What to do after the duration expires: `"auto"` or `"mute"`.
    pub then: String,
}

/// A single policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    pub match_: KeywordMatch,
    pub action: PolicyAction,
}

/// Policy engine stub.
#[derive(Debug, Default)]
pub struct PolicyEngine {
    policies: Vec<Policy>,
}

impl PolicyEngine {
    pub fn new(policies: Vec<Policy>) -> Self {
        Self { policies }
    }
}

impl PolicyEngineTrait for PolicyEngine {
    fn evaluate(&self, keyword: &str, _confidence: f64) -> VolumeDecision {
        for policy in &self.policies {
            if policy.match_.keywords.iter().any(|k| k == keyword) {
                return VolumeDecision::SetVolume {
                    volume: policy.action.volume,
                    duration_seconds: policy.action.duration_seconds,
                };
            }
        }
        VolumeDecision::Default
    }
}

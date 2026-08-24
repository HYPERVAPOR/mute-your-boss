use myb_core::{PolicyEngine as PolicyEngineTrait, VolumeDecision};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Minimum interval between two distinct decisions for the same policy.
/// Hits closer than this are considered KWS jitter and are suppressed.
const DEBOUNCE_MS: i64 = 200;

/// A keyword match configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordMatch {
    pub keywords: Vec<String>,
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

fn default_threshold() -> f64 {
    0.25
}

/// An action to execute when a policy matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    /// Volume level in the range [0, 100].
    pub volume: u32,
    /// How long to keep the volume after the last hit, in seconds.
    pub duration_seconds: u32,
    /// What to do after the duration expires: `"auto"` or `"mute"`.
    #[serde(default = "default_then")]
    pub then: String,
}

fn default_then() -> String {
    "mute".to_string()
}

/// A single policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    #[serde(rename = "match")]
    pub match_: KeywordMatch,
    pub action: PolicyAction,
}

/// Top-level container for a YAML policy file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySet {
    pub policies: Vec<Policy>,
}

impl PolicySet {
    /// Parse a policy set from YAML text.
    pub fn from_yaml(text: &str) -> anyhow::Result<Self> {
        let set: PolicySet = serde_yaml::from_str(text)?;
        set.validate()?;
        Ok(set)
    }

    /// Load a policy set from a YAML file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_yaml(&text)
    }

    /// Validate all policies.
    pub fn validate(&self) -> anyhow::Result<()> {
        let mut names = HashSet::new();
        for policy in &self.policies {
            anyhow::ensure!(!policy.name.is_empty(), "policy name must not be empty");
            anyhow::ensure!(
                names.insert(policy.name.clone()),
                "duplicate policy name: {}",
                policy.name
            );
            anyhow::ensure!(
                !policy.match_.keywords.is_empty(),
                "policy '{}' must have at least one keyword",
                policy.name
            );
            for kw in &policy.match_.keywords {
                anyhow::ensure!(
                    !kw.is_empty(),
                    "policy '{}' contains an empty keyword",
                    policy.name
                );
            }
            anyhow::ensure!(
                (0.0..=1.0).contains(&policy.match_.threshold),
                "policy '{}' threshold must be in [0, 1], got {}",
                policy.name,
                policy.match_.threshold
            );
            anyhow::ensure!(
                policy.action.volume <= 100,
                "policy '{}' volume must be in [0, 100], got {}",
                policy.name,
                policy.action.volume
            );
            anyhow::ensure!(
                policy.action.duration_seconds > 0,
                "policy '{}' duration_seconds must be > 0",
                policy.name
            );
            anyhow::ensure!(
                policy.action.then == "auto" || policy.action.then == "mute",
                "policy '{}' then must be 'auto' or 'mute', got {}",
                policy.name,
                policy.action.then
            );
        }
        Ok(())
    }
}

/// Policy engine implementation.
#[derive(Debug, Default)]
pub struct PolicyEngine {
    policies: Vec<Policy>,
    /// Last timestamp (ms) each policy produced a decision.
    last_hit_ms: HashMap<String, i64>,
}

impl PolicyEngine {
    pub fn new(policies: Vec<Policy>) -> Self {
        Self {
            policies,
            last_hit_ms: HashMap::new(),
        }
    }

    /// Load policies from YAML text.
    pub fn from_yaml(text: &str) -> anyhow::Result<Self> {
        let set = PolicySet::from_yaml(text)?;
        Ok(Self::new(set.policies))
    }

    /// Load policies from a YAML file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let set = PolicySet::from_file(path)?;
        Ok(Self::new(set.policies))
    }

    /// Return the loaded policies.
    pub fn policies(&self) -> &[Policy] {
        &self.policies
    }
}

/// Extract the display label from a keyword spec.
///
/// A spec may be either `DISPLAY=PHONEMES` (used to build the KWS vocabulary)
/// or just `DISPLAY`.  This function returns the `DISPLAY` part.
pub fn keyword_display(spec: &str) -> &str {
    spec.split_once('=').map(|(d, _)| d).unwrap_or(spec).trim()
}

impl PolicyEngineTrait for PolicyEngine {
    fn evaluate(&mut self, keyword: &str, confidence: f64, timestamp_ms: i64) -> VolumeDecision {
        for policy in &self.policies {
            let matched = policy
                .match_
                .keywords
                .iter()
                .any(|k| keyword_display(k) == keyword);
            if !matched || confidence < policy.match_.threshold {
                continue;
            }

            let duration_ms = i64::from(policy.action.duration_seconds) * 1000;
            let last = self.last_hit_ms.get(&policy.name).copied();

            return if last.is_none_or(|t| timestamp_ms - t > duration_ms) {
                // New activation window.
                self.last_hit_ms.insert(policy.name.clone(), timestamp_ms);
                VolumeDecision::SetVolume {
                    volume: policy.action.volume,
                    duration_seconds: policy.action.duration_seconds,
                }
            } else if timestamp_ms - last.unwrap() < DEBOUNCE_MS {
                // Too soon after the last decision: suppress noise.
                VolumeDecision::Default
            } else {
                // Within the active window but past the debounce: renew duration.
                self.last_hit_ms.insert(policy.name.clone(), timestamp_ms);
                VolumeDecision::Renew
            };
        }
        VolumeDecision::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_yaml() -> &'static str {
        r#"
policies:
  - name: unmute-on-light-up
    match:
      keywords:
        - "LIGHT_UP=L AY1 T AH1 P"
        - "演员=y ǎn y uán"
      threshold: 0.25
    action:
      volume: 100
      duration_seconds: 5
      then: mute
  - name: quiet-on-lovely-child
    match:
      keywords:
        - "LOVELY_CHILD=L AH1 V L IY0 CH AY1 L D"
      threshold: 0.5
    action:
      volume: 20
      duration_seconds: 3
      then: auto
"#
    }

    #[test]
    fn parse_yaml() {
        let engine = PolicyEngine::from_yaml(sample_yaml()).unwrap();
        assert_eq!(engine.policies().len(), 2);
        assert_eq!(engine.policies()[0].name, "unmute-on-light-up");
        assert_eq!(engine.policies()[0].match_.keywords.len(), 2);
        assert_eq!(engine.policies()[1].action.volume, 20);
    }

    #[test]
    fn evaluate_respects_threshold() {
        let mut engine = PolicyEngine::from_yaml(sample_yaml()).unwrap();
        assert!(
            matches!(
                engine.evaluate("LIGHT_UP", 0.3, 0),
                VolumeDecision::SetVolume { volume: 100, .. }
            ),
            "confidence above threshold should match"
        );
        assert!(
            matches!(
                engine.evaluate("LIGHT_UP", 0.1, 1000),
                VolumeDecision::Default
            ),
            "confidence below threshold should not match"
        );
    }

    #[test]
    fn evaluate_first_match_wins() {
        let yaml = r#"
policies:
  - name: a
    match:
      keywords: ["X"]
      threshold: 0.0
    action:
      volume: 10
      duration_seconds: 1
      then: mute
  - name: b
    match:
      keywords: ["X"]
      threshold: 0.0
    action:
      volume: 90
      duration_seconds: 1
      then: mute
"#;
        let mut engine = PolicyEngine::from_yaml(yaml).unwrap();
        match engine.evaluate("X", 1.0, 0) {
            VolumeDecision::SetVolume { volume: 10, .. } => {}
            other => panic!("expected first policy to win, got {:?}", other),
        }
    }

    #[test]
    fn evaluate_debounce_and_renew() {
        let yaml = r#"
policies:
  - name: active
    match:
      keywords: ["X"]
      threshold: 0.0
    action:
      volume: 50
      duration_seconds: 1
      then: mute
"#;
        let mut engine = PolicyEngine::from_yaml(yaml).unwrap();

        // First hit starts the active window.
        assert!(matches!(
            engine.evaluate("X", 1.0, 0),
            VolumeDecision::SetVolume { volume: 50, .. }
        ));

        // Hit within the 200ms debounce window is suppressed.
        assert_eq!(engine.evaluate("X", 1.0, 100), VolumeDecision::Default);

        // Hit after debounce but within duration renews the window.
        assert_eq!(engine.evaluate("X", 1.0, 300), VolumeDecision::Renew);

        // Hit after the full duration starts a new active window.
        assert!(matches!(
            engine.evaluate("X", 1.0, 1500),
            VolumeDecision::SetVolume { volume: 50, .. }
        ));
    }

    #[test]
    fn rejects_invalid_volume() {
        let yaml = r#"
policies:
  - name: bad
    match:
      keywords: ["X"]
      threshold: 0.0
    action:
      volume: 101
      duration_seconds: 1
      then: mute
"#;
        assert!(PolicyEngine::from_yaml(yaml).is_err());
    }

    #[test]
    fn rejects_duplicate_names() {
        let yaml = r#"
policies:
  - name: dup
    match:
      keywords: ["X"]
      threshold: 0.0
    action:
      volume: 10
      duration_seconds: 1
      then: mute
  - name: dup
    match:
      keywords: ["Y"]
      threshold: 0.0
    action:
      volume: 20
      duration_seconds: 1
      then: mute
"#;
        assert!(PolicyEngine::from_yaml(yaml).is_err());
    }
}

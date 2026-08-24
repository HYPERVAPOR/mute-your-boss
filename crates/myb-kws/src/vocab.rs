use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// A single keyword entry for a sherpa-onnx keyword spotting model.
///
/// The `display` field is the label returned by the engine (the text after `@`).
/// The `phonemes` field is the token sequence the model expects; for the
/// zipformer-zh-en model this is either ARPAbet (English) or pinyin with tones
/// (Chinese).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeywordEntry {
    pub display: String,
    pub phonemes: String,
}

impl KeywordEntry {
    /// Parse a keyword specification string.
    ///
    /// Supported forms:
    /// - `"LIGHT_UP=L AY1 T AH1 P"` → display `LIGHT_UP`, phonemes `L AY1 T AH1 P`
    /// - `"yǎnyuán=演员"` is **not** handled here; use the `(phonemes, display)` form.
    /// - `"演员"` → display `演员`, phonemes `演员` (pass-through; only useful if the
    ///   model can consume raw text, which the current model cannot).
    ///
    /// For this milestone the caller is expected to supply pre-tokenized
    /// phonemes.  Automatic g2p / pinyin conversion is tracked separately.
    pub fn parse(spec: &str) -> anyhow::Result<Self> {
        let spec = spec.trim();
        anyhow::ensure!(!spec.is_empty(), "empty keyword spec");

        let (display, phonemes) = if let Some((d, p)) = spec.split_once('=') {
            (d.trim().to_string(), p.trim().to_string())
        } else {
            let s = spec.to_string();
            (s.clone(), s)
        };

        anyhow::ensure!(!display.is_empty(), "empty keyword display");
        anyhow::ensure!(
            !phonemes.is_empty(),
            "empty keyword phonemes for '{}'",
            display
        );

        Ok(Self { display, phonemes })
    }

    /// Format this entry in the sherpa-onnx keywords.txt format.
    pub fn to_sherpa_onnx_line(&self) -> String {
        format!("{} @{}", self.phonemes, self.display)
    }
}

/// A vocabulary of keywords ready to be serialized for sherpa-onnx.
#[derive(Debug, Default, Clone)]
pub struct KeywordVocab {
    entries: BTreeSet<KeywordEntry>,
}

impl KeywordVocab {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a keyword entry, replacing any existing entry with the same display.
    pub fn add(&mut self, entry: KeywordEntry) {
        // BTreeSet uses Ord, which compares display first then phonemes.
        // Remove any entry with the same display to allow updates.
        self.entries.retain(|e| e.display != entry.display);
        self.entries.insert(entry);
    }

    /// Add a keyword from a spec string (see [`KeywordEntry::parse`]).
    pub fn add_str(&mut self, spec: &str) -> anyhow::Result<()> {
        self.add(KeywordEntry::parse(spec)?);
        Ok(())
    }

    /// Add multiple specs at once.
    pub fn add_many<I, S>(&mut self, specs: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for spec in specs {
            self.add_str(spec.as_ref())?;
        }
        Ok(())
    }

    /// Build a vocabulary from the keywords declared in `myb_policy::Policy` items.
    ///
    /// Each policy keyword must be in `display=phonemes` form.  Plain display
    /// strings are accepted but will be passed through as their own phonemes,
    /// which is usually wrong for the zipformer-zh-en model.
    pub fn from_policies(policies: &[myb_policy::Policy]) -> anyhow::Result<Self> {
        let mut vocab = Self::new();
        for policy in policies {
            for kw in &policy.match_.keywords {
                vocab.add_str(kw)?;
            }
        }
        Ok(vocab)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> impl Iterator<Item = &KeywordEntry> {
        self.entries.iter()
    }

    /// Serialize the vocabulary to the sherpa-onnx keywords.txt format.
    pub fn to_sherpa_onnx_string(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            let _ = writeln!(out, "{}", entry.to_sherpa_onnx_line());
        }
        out
    }

    /// Write the vocabulary to a file and return the path.
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<PathBuf> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.to_sherpa_onnx_string())?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_display_equals_phonemes() {
        let e = KeywordEntry::parse("LIGHT_UP=L AY1 T AH1 P").unwrap();
        assert_eq!(e.display, "LIGHT_UP");
        assert_eq!(e.phonemes, "L AY1 T AH1 P");
    }

    #[test]
    fn parse_plain_pass_through() {
        let e = KeywordEntry::parse("演员").unwrap();
        assert_eq!(e.display, "演员");
        assert_eq!(e.phonemes, "演员");
    }

    #[test]
    fn to_sherpa_onnx_line() {
        let e = KeywordEntry::parse("LIGHT_UP=L AY1 T AH1 P").unwrap();
        assert_eq!(e.to_sherpa_onnx_line(), "L AY1 T AH1 P @LIGHT_UP");
    }

    #[test]
    fn vocab_dedupes_by_display() {
        let mut vocab = KeywordVocab::new();
        vocab.add_str("LIGHT_UP=L AY1 T AH1 P").unwrap();
        vocab.add_str("LIGHT_UP=L AY1 T AH0 P").unwrap();
        assert_eq!(vocab.len(), 1);
        assert_eq!(vocab.entries().next().unwrap().phonemes, "L AY1 T AH0 P");
    }
}

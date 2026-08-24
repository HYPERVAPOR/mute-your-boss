use myb_core::KeywordVocab;
use std::fs;
use std::path::{Path, PathBuf};

/// Extension helpers for building a KWS vocabulary from raw specs.
pub trait KeywordVocabExt {
    /// Build a vocabulary from a list of keyword spec strings.
    ///
    /// Each spec uses the format `DISPLAY=PHONEMES`, e.g.
    /// `"LIGHT_UP=L AY1 T AH1 P"` or `"演员=y ǎn y uán"`.
    fn from_specs<I, S>(specs: I) -> anyhow::Result<Self>
    where
        Self: Sized,
        I: IntoIterator<Item = S>,
        S: AsRef<str>;

    /// Write the vocabulary to a file and return the path.
    fn write_to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<PathBuf>;
}

impl KeywordVocabExt for KeywordVocab {
    fn from_specs<I, S>(specs: I) -> anyhow::Result<Self>
    where
        Self: Sized,
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut vocab = KeywordVocab::new();
        for spec in specs {
            vocab.add_str(spec.as_ref())?;
        }
        Ok(vocab)
    }

    fn write_to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<PathBuf> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, self.to_sherpa_onnx_string())?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_specs_builds_vocab() {
        let vocab =
            KeywordVocab::from_specs(["LIGHT_UP=L AY1 T AH1 P", "演员=y ǎn y uán"]).unwrap();
        assert_eq!(vocab.len(), 2);
    }
}

use rhai::EvalAltResult;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScriptLangError {
    #[error("{message}")]
    Message { message: String },
    #[error("xml parse error: {0}")]
    XmlParse(#[from] roxmltree::Error),
    #[error("rhai parse error: {0}")]
    RhaiParse(#[from] rhai::ParseError),
    #[error("rhai eval error: {0}")]
    RhaiEval(#[from] Box<EvalAltResult>),
}

impl ScriptLangError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }
}

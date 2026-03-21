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

#[cfg(test)]
mod tests {
    use super::ScriptLangError;

    #[test]
    fn message_constructor_and_display_work() {
        let error = ScriptLangError::message("hello");
        let owned = ScriptLangError::message(String::from("owned"));

        assert_eq!(error.to_string(), "hello");
        assert!(matches!(
            error,
            ScriptLangError::Message { message } if message == "hello"
        ));
        assert!(matches!(
            owned,
            ScriptLangError::Message { message } if message == "owned"
        ));
    }

    #[test]
    fn xml_parse_error_conversion_works() {
        let error = roxmltree::Document::parse("<module>")
            .expect_err("xml should be invalid")
            .into();

        assert!(matches!(error, ScriptLangError::XmlParse(_)));
        assert!(error.to_string().contains("xml parse error"));
    }

    #[test]
    fn rhai_parse_error_conversion_works() {
        let error = rhai::Engine::new()
            .compile("let =")
            .expect_err("rhai parse should fail")
            .into();

        assert!(matches!(error, ScriptLangError::RhaiParse(_)));
        assert!(error.to_string().contains("rhai parse error"));
    }

    #[test]
    fn rhai_eval_error_conversion_works() {
        let error = rhai::Engine::new()
            .eval_expression::<i64>("unknown_name")
            .expect_err("rhai eval should fail")
            .into();

        assert!(matches!(error, ScriptLangError::RhaiEval(_)));
        assert!(error.to_string().contains("rhai eval error"));
    }
}

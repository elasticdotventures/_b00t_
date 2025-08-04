/* file src/typescript_wasm.rs */

use wasm_bindgen::prelude::*;
use std::convert::TryFrom;
use wasm_bindgen::JsValue;
use crate::KmdLine;


#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct KmdLineWasm {
    verb: Option<String>,
    params: Option<String>,  // JSON string
    content: Option<String>,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl KmdLineWasm {
    pub fn new() -> KmdLineWasm {
        KmdLineWasm {
            verb: None,
            params: None,
            content: None,
        }
    }

    pub fn verb(&self) -> Option<String> {
        self.verb.clone()
    }

    pub fn params(&self) -> Option<String> {
        self.params.clone()
    }

    pub fn content(&self) -> Option<String> {
        self.content.clone()
    }

    pub fn parse(input: &str) -> Result<KmdLineWasm, JsValue> {
        let mut input_str = input;
        match KmdLine::parse(&mut input_str) {
            Ok(kmdline) => KmdLineWasm::try_from(&kmdline),
            Err(e) => Err(JsValue::from_str(&format!("Parse error: {:?}", e))),
        }
    }
}

// impl From<&KmdLine<'_>> for KmdLineWasm {
//     fn from(kmdline: &KmdLine) -> Self {
//         let verb = kmdline.verb.clone();
//         let content = kmdline.content.clone();

//         let params = kmdline.params.as_ref().map(|kmdparams| {
//             // Serialize the HashMap to a JSON string
//             serde_json::to_string(&kmdparams.kvs).unwrap_or_default()
//         });

//         KmdLineWasm {
//             verb,
//             params,
//             content,
//         }
//     }
// }

#[cfg(target_arch = "wasm32")]
impl TryFrom<&KmdLine<'_>> for KmdLineWasm {
    type Error = JsValue;

    fn try_from(kmdline: &KmdLine) -> Result<Self, Self::Error> {
        let verb = kmdline.verb.clone();
        let content = kmdline.content.clone();

        let params = kmdline.params.as_ref()
            .map(|kmdparams| serde_json::to_string(&kmdparams.kvs).unwrap_or_default())
            .filter(|s| !s.is_empty()); // Only Some if string is not empty

        Ok(KmdLineWasm {
            verb,
            params,
            content,
        })
    }
}


/* *********************** */

#[cfg(target_arch = "wasm32")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test__WASM__parse_valid_input() {
        let input = "/verb --param1=value1 --param2=value2";
        let result = KmdLineWasm::parse(input).unwrap();
        assert_eq!(result.verb(), Some("verb".to_string()));
        // Additional assertions for 'params' and 'content'
    }

    #[test]
    fn test__WASM__parse_invalid_input() {
        let input = "invalid input";
        let result = KmdLineWasm::parse(input);
        assert!(result.is_err());
        // You can also test for specific error messages if desired
    }

    #[test]
    fn test__WASM__parse_verb_only() {
        let input = "/onlyverb";
        let result = KmdLineWasm::parse(input).unwrap();
        assert_eq!(result.verb(), Some("onlyverb".to_string()));
        assert!(result.params().is_none());
        assert!(result.content().is_none());
    }

    // Additional tests can be written for other edge cases and scenarios
}


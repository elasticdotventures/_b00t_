/*
file: src/lib.rs

Concepts:
K0mmand is a framework for parsing slash verbs in a chat application.

KmdLine is a single verb with three parts:
    Kmd is the instruction or empty for content
        * verbs are always lowercase
        * verbs ALWAYS begin with a letter then zero or more alphanumeric characters
    params is a vector of KmdParameter
        KmdParameter is a label-value pair
            label is a string that starts with a letter followed by zero or more alphanumeric characters
            value is a KmdValueType
                KmdValueType is a string, number, boolean, user, channel, or tag
                the string can be either quoted single alphanumeric, OR a quoted string
    content is a string

feature-python enables pyo3 bindings
feature-wasm enables typescript-wasm bindings


🤓 Rust-lang winnow is parser+combinator tutorial found here:
* https://docs.rs/winnow/latest/winnow/_tutorial/index.html#modules

Summary of winnow:
    “parsers”, functions that take an input and give back an output
    “combinators”, functions that take parsers and combine them together!

Parsers takes an input & returns a result:
    Ok indicates the parser successfully found what it was looking for; or
    - Parsers do more than just return a binary “success”/“failure” code.
    - On success, the parser will return the processed data. The input will be left pointing to data that still needs processing
    Err indicates the parser could not find what it was looking for.
    - then there are multiple errors that could be returned

Winnow uses the PResult<O> type.
The Ok variant has output: O; whereas the Err variant stores an error.

To combine parsers,
    you'll need a common way to refer to them so you use the
        Parser<I, O, E> trait with Parser::parse_next 💡 this is the primary way to drive parsing forward.
    You’ll note that I and O are parameterized – while most of the examples will be with &str (i.e. parsing a string); they do not have to be strings; nor do they have to be the same type (consider the simple example where I = &str, and O = u64 – this parses a string into an unsigned integer.)

---

Kommand Grammar:
this is a string parser for "/slash" verbs

any string should first be trimmed for whitespace on the front and back.
    after trimming, if found the kommand must begin with a forward slash "/",
    any string which doesn't begin with a / is returned as "content" (it has no verb)

if a kommand is found then parse proceeds to parse the grammar of the verb
here are the rules for parsing a kommand grammar:

zero or more parameters will be found
    --parameters are prefixed by a double dash "--"
    parameters are always alphanumeric
    if a parameter is followed by an = then it will have a value token
    so --parameter or --parameter=value can be returned
    the order of parameters is important and should be preserved in the structures
        a parameter with no value is called a "tag"
        a parameter with a value is a type "kvpair"
    values can be of four types:
        1. string
        2. number
        3. boolean
        4. @user  (a user token begins with a literal "@" followed by a letter, followed by one or more alphanumeric or emoji characters
        5. #channel (a channel token begins with a literal "#" followed by a letter, followed by one or more alphanumeric or emoji characters

    these structures once parsed should be stored in an k0mmand3r result object

Examples (one per line)
```
/verb --param1=value1 --param2=value2 --tag random content
/anotherverb --user=@john_doe --channel=#general
this is just content, no verb!
```

*/

#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]
//use indexmap::IndexMap;

use std::collections::HashMap;
use decimal_rs::Decimal;

#[cfg(feature = "lang-python")]
use pyo3::prelude::*;

use winnow::ascii::{alpha1, alphanumeric0, alphanumeric1, multispace0};
use winnow::combinator::alt; // encapsulates if/then/else ladder pattern
use winnow::combinator::opt; // basic if then else
use winnow::combinator::preceded; // an easy way to discard the prefix, using a provided combinators
use winnow::combinator::Recognize;
use winnow::combinator::WithRecognized;
use winnow::combinator::{delimited, repeat, separated, separated_pair, *};
use winnow::error::ErrMode;
use winnow::error::ErrorKind;
use winnow::error::ParserError;
use winnow::prelude::*;
use winnow::seq;
use winnow::stream::Stream; // choose between two parsers; and we’re happy with either being used.
use winnow::token::one_of; // one_of(('0'..='9', 'a'..='f', 'A'..='F')).parse_next(input)
use winnow::{PResult, Parser};
use winnow::token::take_while;

use serde_json::json;
use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "lang-python")]
mod python;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "lang-python")]
pub use python::k0mmand3r;


#[cfg(target_arch = "wasm32")]
mod typescript_wasm;



/* winnow parsers */
fn parse_prefix_dash2x<'s>(input: &mut &'s str) -> PResult<&'s str> {
    "--".parse_next(input)
}

fn parse_label<'i>(input: &mut &'i str) -> PResult<&'i str> {
    // first character alpha, followed by zero or more alphanumeric
    let label_parser = seq!((alpha1, alphanumeric0));
    label_parser.recognize().parse_next(input)
}

fn parse_value_quoted<'i>(input: &mut &'i str) -> PResult<&'i str> {
    // example: "a" or "1"
    delimited('"', alphanumeric1, '"').parse_next(input)
}

fn parse_value_unquoted<'i>(input: &mut &'i str) -> PResult<&'i str> {
    // example: a 1
    alphanumeric1.parse_next(input)
}

fn parse_value_quote_agnostic<'i>(input: &mut &'i str) -> PResult<&'i str> {
    // example: "a" or "1" or a 1
    alt((
        parse_value_quoted,   // Try parsing a quoted value first
        parse_value_unquoted, // If that fails, try parsing an unquoted value
    ))
    .parse_next(input)
}

fn parse_slashcommand<'i>(input: &mut &'i str) -> PResult<&'i str> {
    // strips the / from a command-verb label
    preceded("/", parse_label).parse_next(input)
}

fn parse_KmdParameter<'i>(input: &mut &'i str) -> PResult<(&'i str, &'i str)> {
    // Parse the prefix "--"
    // separated_pair(parse_label, "=", parse_values).parse_next(input)
    preceded(
        parse_prefix_dash2x,
        separated_pair(
            parse_label,
            opt(delimited(multispace0, '=', multispace0)), // Make value part optional
            opt(parse_value_quote_agnostic),               // Optional value
        ),
    )
    .map(|(label, value)| (label, value.unwrap_or(""))) // Default to empty string if no value
    .parse_next(input)
}

/**
 * Parse the content of a message
 */
fn parse_content<'i>(input: &mut &'i str) -> PResult<&'i str> {
    let trimmed_input = input.trim();

    if trimmed_input.starts_with('/') {
        let error = winnow::error::ContextError::new();
        Err(winnow::error::ErrMode::Backtrack(error))
    } else {
        // Find the start of the trimmed content within the original input
        let start_index = input.find(trimmed_input).unwrap_or(0);
        // Calculate the end index of the trimmed content
        let end_index = start_index + trimmed_input.len();

        // Create a slice of the original input from the start to the end of the trimmed content
        let result = &input[start_index..end_index];

        // Update input to the remaining part after the trimmed content
        *input = &input[end_index..];

        Ok(result)
    }
}


#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct KmdParams<'i> {
    kvs: HashMap<&'i str, &'i str>,
}

impl<'i> KmdParams<'i> {
    pub fn parse(input: &mut &'i str) -> PResult<Self> {
        let kvs =
            separated(0.., parse_KmdParameter, terminated(' ', multispace0)).parse_next(input)?;

        Ok(Self { kvs })
    }
}


/* *********************** */


#[derive(Debug, PartialEq, Serialize)]
pub struct KmdLine<'i> {
    // Verb of the command; None if it's just content
    verb: Option<String>,
    // Parameters of the command; None if there are no parameters
    params: Option<KmdParams<'i>>,
    // Content; None if there is no content
    content: Option<String>,
}

impl<'i> KmdLine<'i> {
    pub fn parse(input: &mut &'i str) -> PResult<Self> {
        let trimmed_input = input.trim();

        if trimmed_input.starts_with('/') {
            // Parse the verb
            let verb = Some(parse_slashcommand(input)?.to_string());

            // Check if the remaining input is empty after parsing the verb
            if input.trim().is_empty() {
                // If yes, return with verb only, no params and content
                return Ok(KmdLine {
                    verb,
                    params: None,
                    content: None,
                });
            }

            // Consume whitespace before parsing params
            let _ = multispace0.parse_next(input)?;

            // Parse parameters
            let params = opt(KmdParams::parse).parse_next(input)?;

            // Consume whitespace before parsing content
            let _ = multispace0.parse_next(input)?;

            // Parse remaining content
            let content = opt(parse_content).parse_next(input)?.map(|c| c.to_string());

            Ok(KmdLine {
                verb,
                params,
                content,
            })
        } else {
            // If it's not a verb, treat the entire input as content
            // parse_content(input).map(|content| KmdLine::Content(content.to_string()))
            let content = Some(parse_content(input)?.to_string());
            Ok(KmdLine {
                verb: None,
                params: None,
                content,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_without_leading_slash() {
        let mut input = "this is just content";
        let expected = "this is just content";
        let result = parse_content(&mut input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn content_with_leading_slash() {
        let mut input = "/not just content";
        let result = parse_content(&mut input);
        assert!(result.is_err()); // Expect an error because it starts with '/'
    }

    #[test]
    fn content_with_whitespace() {
        let mut input = "   leading and trailing spaces   ";
        let expected = "leading and trailing spaces";
        let result = parse_content(&mut input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_prefix_dash2x() {
        let input = "--";
        let actual = parse_prefix_dash2x.parse(input).unwrap();
        let expected = "--";
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_label() {
        let input = "mylabel";
        let actual = parse_label.parse(input).unwrap();
        let expected = "mylabel";
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_labelWithNumber() {
        let input = "mylabel1";
        let actual = parse_label.parse(input).unwrap();
        let expected = "mylabel1";
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_value_quoted() {
        let input = r#""40""#;
        let actual = parse_value_quoted.parse(input).unwrap();
        let expected = "40";
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_value_unquoted() {
        let input = r#"40"#;
        let actual = parse_value_unquoted.parse(input).unwrap();
        let expected = "40";
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_slashcommand() {
        let input = r#"/command"#;
        let actual = parse_slashcommand.parse(input).unwrap();
        let expected = "command";
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_isolatedLabel() {
        let input = r#"--mylabel"#;
        let actual = parse_KmdParameter.parse(input).unwrap();
        let expected = ("mylabel", "");
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_dash2xlabelvalueQUOTED_for_KmdParameter() {
        let input = r#"--mylabel="40""#;
        let actual = parse_KmdParameter.parse(input).unwrap();
        let expected = ("mylabel", "40");
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_dash2xlabelvalueUNQUOTED_for_KmdParameter() {
        let input = r#"--mylabel=40"#;
        let actual = parse_KmdParameter.parse(input).unwrap();
        let expected = ("mylabel", "40");
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_parametersOne() {
        //let input = r#"--mylabel1="10" --mylabel2=20"#;
        let input = r#"--onelabel="10""#;
        let actual = KmdParams::parse.parse(input).unwrap();
        let expected = KmdParams {
            kvs: HashMap::from([("onelabel", "10")]),
        };

        assert_eq!(actual, expected)
    }

    #[test]
    fn test_parametersMany() {
        //let input = r#"--mylabel1="10" --mylabel2=20"#;
        let input = r#"--mylabel="10" --yourlabel=20"#;
        let actual = KmdParams::parse.parse(input).unwrap();
        let expected = KmdParams {
            kvs: HashMap::from([("mylabel", "10"), ("yourlabel", "20")]),
        };

        assert_eq!(actual, expected)
    }

    #[test]
    fn test_kommand_WithContent() {
        let mut input = r#"/save --mylabel=myvalue remaining content"#;
        let actual = KmdLine::parse(&mut input).unwrap();
        let expected = KmdLine {
            verb: Some("save".to_string()),
            params: Some(KmdParams {
                kvs: HashMap::from([("mylabel", "myvalue")]),
            }),
            content: Some("remaining content".to_string()),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_kommand_ContentOnly() {
        let mut input =
            r#"lots of content with\nhard returns\nand embedded verbs and --parameters like /save"#;
        let expected = KmdLine {
            verb: None,
            params: None,
            content: Some(input.to_string()),
        };

        let actual = KmdLine::parse(&mut input).unwrap();
        assert_eq!(actual, expected);
    }


    #[test]
    fn test_kommand_VerbOnly() {
        let mut input =
            r#"/verbonly"#;
        let expected = KmdLine {
            verb: Some("verbonly".to_string()),
            params: None,
            content: None
        };

        let actual = KmdLine::parse(&mut input).unwrap();
        assert_eq!(actual, expected);
    }

}


/*

FINAL INSTRUCTIONS:
any errors will be pasted below solving those is 👍🏻,
also solving any // TODO: blocks in the code!

 */

//! Directives for TaggedValues
//!
//! Includes helper functions to break apart TaggedValue parsing

/* IMPORTS */
use indicatif::ProgressBar;
use serde::Deserialize;
use serde_yaml::{value::TaggedValue, Deserializer, Mapping, Sequence, Value};
use std::{cell::RefCell, fmt, sync::Arc};

/* LOCAL IMPORTS */
use crate::{debug, error, info, warn, Options, PageNode, Parser};

/* DIRECTIVES */
/// Macro to automate parsing a Value into a boxed str given a target and Value
///
/// $parent: Arc<RefCell<PageNode>>
/// $value: &serde_yaml::Value
#[macro_export]
macro_rules! parse_value {
    ($parent:expr, $value:expr) => {{
        let child = Arc::new(RefCell::new(PageNode::new($parent.borrow().o.clone())));
        child.borrow_mut().set_parent($parent.clone());
        Parser::add_value(child.clone(), $value);
        format!("{}", child.borrow()).into_boxed_str()
    }};
}

/// If a value exists / is not an empty string, do something. Otherwise, do something else (if it exists)
///
/// Usage:
/// ```YAML
/// !IF [condition, exec if true, ?exec if false]
/// ```
/// Where `?exec if false` is optional
pub fn if_else(target: Arc<RefCell<PageNode>>, tv: &TaggedValue) {
    debug!(target.borrow().o, "Evaluating conditional...");
    match &tv.value {
        Value::Sequence(seq) => {
            if seq.len() >= 2 && seq.len() <= 3 {
                let condition = parse_value!(target, &seq[0]).to_string();
                match condition.as_str() {
                    "" => {
                        // exec 'else' block
                        if seq.len() == 3 {
                            Parser::add_value(target.clone(), &seq[2]);
                        }
                    }
                    _ => {
                        // exec 'if' block
                        Parser::add_value(target.clone(), &seq[1]);
                    }
                }
            }
        }
        _ => (),
    }
    error!(
        target.borrow().o,
        "Incorrectly formatted conditional: {}",
        value_tostring(&tv.value)
    );
}

/// Define a variable from YAML
///
/// Define a variable in YAML into a target PageNode
/// Usage:
/// ```YAML
/// !DEF: [key, val]
/// ```
pub fn def(target: Arc<RefCell<PageNode>>, tv: &TaggedValue) {
    if tv.value.is_sequence() {
        let s = tv.value.as_sequence().unwrap();
        if s.len() == 2 {
            let kstr = parse_value!(target, &s[0]);
            let vstr = parse_value!(target, &s[1]);
            info!(target.borrow().o, "Registering variable {kstr}...");
            target.borrow_mut().register_var(kstr, vstr);
        }
    } else {
        error!(
            target.borrow().o,
            "Invalid arguments to !DEF directive: {}",
            value_tostring(&tv.value)
        )
    }
}

/// Iterate over some data provided through YAML according to a template
///
/// Usage:
/// ```YAML
/// !FOREACH [
///   [x, y, ..., n],              # Variable names for use in template
///   "${x} ${y} ${n}",            # Template for values to be inserted into
///   [xval, yval, ..., nval],     # One set of values to insert into the template
///   [xval2, yval2, ..., zval2],  # Another set of values
/// ]
/// ```
pub fn foreach(target: Arc<RefCell<PageNode>>, tv: &TaggedValue) {
    info!(target.borrow().o, "Looping into !FOREACH directive...");
    match &tv.value {
        Value::Sequence(foreach) => 'invalid_foreach: {
            // ensure preconditions
            if foreach.len() < 3 || !foreach[0].is_sequence() {
                break 'invalid_foreach;
            };
            let keys = foreach[0]
                .as_sequence()
                .unwrap()
                .iter()
                .map(|k| parse_value!(target, k))
                .collect::<Vec<Box<str>>>();

            // iterate over all subsequences in the rest of foreach
            for values in foreach.iter().skip(2) {
                match values {
                    Value::Sequence(seq) => {
                        if seq.len() != keys.len() {
                            break 'invalid_foreach;
                        }
                        // create new child
                        let child =
                            Arc::new(RefCell::new(PageNode::new(target.borrow().o.clone())));
                        child.borrow_mut().set_parent(target.clone());
                        target.borrow_mut().add_child(child.clone());
                        // register vars
                        seq.iter().enumerate().for_each(|(i, v)| {
                            let vstr = parse_value!(child, v);
                            child
                                .borrow_mut()
                                .register_var(keys[i].clone().into(), vstr.into());
                        });
                        // apply template string
                        Parser::add_value(child, &foreach[1]);
                    }
                    _ => (),
                }
            }
            return;
        }
        _ => (),
    }
    // if fail
    error!(
        target.borrow().o,
        "Invalid arguments to !FOREACH directive: {}",
        value_tostring(&tv.value)
    );
}

/// Convert a serde_yaml::Value to a String
fn value_tostring(val: &Value) -> String {
    return match val {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.to_string(),
        Value::Sequence(seq) => {
            format!(
                "[{}]",
                seq.iter()
                    .map(|i| value_tostring(i) + ",")
                    .collect::<String>()
            )
        }
        Value::Mapping(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| match v {
                    Value::Sequence(_) | Value::Mapping(_) =>
                        format!(r#""{}":{},"#, value_tostring(k), value_tostring(v)),
                    _ => format!(r#""{}":"{}","#, value_tostring(k), value_tostring(v)),
                })
                .collect::<String>()
        ),
        Value::Tagged(t) => format!("{} {}", t.tag, value_tostring(&t.value).as_str()),
    };
}

/* TESTS */
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Args, Parser};
    use clap::Parser as ClapParser;
    use serde_yaml::Number;

    /// Ensure Parser can handle !FOREACH and follow its directives
    #[test]
    fn test_foreach() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-s"]).build_options());
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
!FOREACH [
  [x],
  "<div>{x}</div>",
  [text1],
  [text2],
  [text3],
]
"#,
        );
        assert_eq!(
            format!("{}", p),
            "<div>text1</div><div>text2</div><div>text3</div>"
        );

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
!FOREACH [
  [x, y, z],
  div: '{x}{y}{z}',
  [text1, abc, 123],
  [text2, def, 456],
  [text3, ghi, 789],
]
---
- !FOREACH [[invalid,],]
- !FOREACH [[nonamatching, keys, length,], '', [a,],]
- !FOREACH not a sequence
- !FOREACH [[x], '', not a sequence,]
"#,
        );
        assert_eq!(
            format!("{}", p),
            "<div>text1abc123</div><div>text2def456</div><div>text3ghi789</div>"
        );
    }

    /// Ensure Parser can handle !IF and follow its directives
    #[test]
    fn test_if() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-s"]).build_options());
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
- !DEF [x, y]
- !IF ['{x}', z]
- !IF ['{y}', x, q]
- !IF [
    '',
    [se, qu, en, ce],
    {p: text,},
  ]
- !IF [a, b, c, d, e, f, g]
- !IF not a sequence
"#,
        );

        assert_eq!(format!("{}", p), "zq<p>text</p>");
    }

    /// Ensure Parser can handle !DEF and follow its directives
    #[test]
    fn test_def() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-s"]).build_options());
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
- !DEF [x, y]
- '{x}'
- a:
    - !DEF [x, z]
    - '{x}'
- [!DEF [x, w], '{x}']
- '{x}'
- !DEF [incorrect, size, arguments, aaaaaaa,]
- !DEF this is not a sequence
"#,
        );

        assert_eq!(format!("{}", p), "y<a>z</a>wy");
    }

    #[test]
    fn test_value_tostring() {
        // primitives
        assert_eq!(value_tostring(&Value::Null), "NULL");
        assert_eq!(value_tostring(&Value::Bool(true)), "true");
        assert_eq!(value_tostring(&Value::Bool(false)), "false");
        assert_eq!(value_tostring(&Value::Number(Number::from(1234))), "1234");
        assert_eq!(
            value_tostring(&Value::Number(Number::from(1234.56))),
            "1234.56"
        );
        assert_eq!(value_tostring(&Value::String("asdf".to_string())), "asdf");

        // mapping
        let m: Value = serde_yaml::from_str(r#"{a: b, 1: cdefg, h: [i, j, k]}"#).unwrap();
        assert_eq!(value_tostring(&m), r#"{"a":"b","1":"cdefg","h":[i,j,k,],}"#);

        // tagged
        let t: Value = serde_yaml::from_str(r#"!TAG value"#).unwrap();
        assert_eq!(value_tostring(&t), "!TAG value");

        // sequence
        let mut v: Vec<Value> = Vec::new();
        v.push(Value::Null);
        v.push(123.into());
        v.push("abc".into());
        v.push(true.into());
        v.push(v.clone().into());
        v.push(m.into());
        v.push(t);
        assert_eq!(
            value_tostring(&Value::Sequence(v)),
            r#"[NULL,123,abc,true,[NULL,123,abc,true,],{"a":"b","1":"cdefg","h":[i,j,k,],},!TAG value,]"#
        );
    }
}

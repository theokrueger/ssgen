//! Directives for TaggedValues
//!
//! Includes helper functions to break apart TaggedValue parsing

/* IMPORTS */
use indicatif::ProgressBar;
use serde::Deserialize;
use serde_yaml::{value::TaggedValue, Deserializer, Mapping, Sequence, Value};
use std::{cell::RefCell, fmt, sync::Arc};

/* LOCAL IMPORTS */
use crate::{debug, error, info, warn, Options, PageNode};

pub fn def(target: Arc<RefCell<PageNode>>, tv: &TaggedValue) {
    if tv.value.is_sequence() {
        let s = tv.value.as_sequence().unwrap();
        if s.len() == 2 {
            let kstr = target.borrow().parse_string(s[0].as_str().unwrap().into());
            let vstr = target.borrow().parse_string(s[1].as_str().unwrap().into());
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

/// convert a serde_yaml::Value to a String
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

    /// Ensure Parser can handle TaggedValues and follow their directives
    #[test]
    fn test_tagged() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-d"]).build_options());
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
- !DEF [x, y]
- '{x}'
"#,
        );

        assert_eq!(format!("{}", p), "y");
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

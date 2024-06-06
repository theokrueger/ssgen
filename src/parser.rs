//! Parser that constructs PageNodes from YAML
//!
//! Deserialises YAML into HashMaps and arrays, then reads it and constructs a tree of PageNodes
//! ```
//! // todo
//! ```
/* IMPORTS */
use indicatif::ProgressBar;
use serde::Deserialize;
use serde_yaml::{value::TaggedValue, Deserializer, Mapping, Sequence, Value};
use std::{
    cell::RefCell,
    collections::{HashMap, LinkedList},
    fmt,
    sync::Arc,
};

/* LOCAL IMPORTS */
use crate::{debug, error, info, warn, Options, PageNode};

/* PARSER */
pub struct Parser {
    /// Global Options struct
    o: Arc<Options>,

    /// The root node of the PageNode tree representing the entire HTML document
    root_node: Arc<RefCell<PageNode>>,

    /// indicatif ProgressBar that gets incremented once parsing is completed
    progressbar: Option<Arc<ProgressBar>>,
}

impl Parser {
    /// Parse a string into the PageNode
    pub fn parse_yaml(&mut self, yaml: &str) {
        debug!(self.o, "Parsing YAML");
        for doc in Deserializer::from_str(yaml) {
            match Value::deserialize(doc) {
                Ok(input) => Parser::add_value(self.root_node.clone(), &input),
                Err(e) => panic!("Error while parsing YAML: {}", e),
            }
        }
        // increment progressbar after completion
        match &self.progressbar {
            Some(pb) => {
                pb.inc(1);
                pb.tick();
            }
            None => (),
        }
    }

    /// Create a new, empty Parser
    pub fn new(o: Arc<Options>) -> Parser {
        debug!(o, "Creating new Parser...");
        return Parser {
            root_node: Arc::new(RefCell::new(PageNode::new(o.clone()))),
            progressbar: None,
            o: o,
        };
    }

    /// Add a progressbar to the struct
    pub fn add_progressbar(&mut self, pb: Arc<ProgressBar>) {
        self.progressbar = Some(pb);
    }

    /// Add a ```serde_yaml::Value``` into self
    ///
    /// Primitive `Value`s will just be converted to strings
    /// Special cases for `Sequence`s, `Mapping`s, and `TaggedValue`s
    /// - `Sequence`: Create a Pagenode for each element (except for metadata)
    /// - `Mapping`: Convert Mapping into PageNode
    /// - `TaggedValue`: Follow the !TAG directive
    /// TODO cleanup the function
    fn add_value(target: Arc<RefCell<PageNode>>, val: &Value) {
        match val {
            // primitives just get read as strings
            Value::Null => (),
            Value::Bool(b) => target.borrow_mut().add_content(b.to_string().into()),
            Value::Number(n) => target.borrow_mut().add_content(n.to_string().into()),
            Value::String(s) => target.borrow_mut().add_content(s.clone().into_boxed_str()), // TODO slow

            // sequence gets flattened and all elements become their own PageNode in target's children
            // UNLESS the element in the sequence is a mapping where key starts with a "_" (metadata)
            Value::Sequence(seq) => Parser::parse_seq(target, seq),

            // key becomes name of new pagenode and value becomes child(ren) or content
            Value::Mapping(map) => {
                Parser::parse_map(target, map);
            }
            Value::Tagged(t) => (),
        };
    }

    /// Create a PageNode for each element and add it as a nameless child
    /// If an element in the sequence would be metadata, instead add it to the parent's metadata
    /// This is achieved by just forwarding mappings to parse_map
    fn parse_seq(target: Arc<RefCell<PageNode>>, seq: &Sequence) {
        for i in seq.iter() {
            match i {
                Value::Mapping(map) => {
                    map.iter().for_each(|(k, v)| {
                        // parse k as string only TODO this might cause issues
                        // can a key Value not be a string?
                        debug_assert!(k.is_string()); // TODO
                        let kstr = target.borrow().parse_string(k.as_str().unwrap().into());

                        if kstr.len() > 0 && &kstr[..1] == "_" {
                            // leading underscore for key indicates metadata
                            let mut child =
                                Arc::new(RefCell::new(PageNode::new(target.borrow().o.clone())));
                            Parser::add_value(child.clone(), v);
                            let vstr = format!("{}", child.borrow());
                            target
                                .borrow_mut()
                                .add_metadata((kstr[1..].into(), vstr.into()));
                        }
                    });
                }
                _ => (),
            };
            let mut child = Arc::new(RefCell::new(PageNode::new(target.borrow().o.clone())));
            child.borrow_mut().set_parent(target.clone());
            target.borrow_mut().add_child(child.clone());
            Parser::add_value(child.clone(), i);
        }
    }

    /// Create a PageNode for Mapping element and add it to target
    fn parse_map(target: Arc<RefCell<PageNode>>, map: &Mapping) {
        map.iter().for_each(|(k, v)| {
            let mut child = Arc::new(RefCell::new(PageNode::new(target.borrow().o.clone())));
            // parse k as string only TODO this might cause issues
            // can a key Value not be a string?
            debug_assert!(k.is_string()); // TODO
            let kstr = target.borrow().parse_string(k.as_str().unwrap().into());

            if kstr.len() > 0 && &kstr[..1] == "_" {
                // leading underscore for key indicates metadata
                Parser::add_value(child.clone(), v);
                let vstr = format!("{}", child.borrow());
                target
                    .borrow_mut()
                    .add_metadata((kstr[1..].into(), vstr.into()));
            } else {
                // no leading unnderscore means parse as normal data
                child.borrow_mut().set_parent(target.clone());
                child.borrow_mut().set_name(kstr.into());
                Parser::add_value(child.clone(), v);
                target.borrow_mut().add_child(child.clone());
            }
        });
    }
}

impl fmt::Display for Parser {
    /// Resolve the PageNode into a String
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        return write!(f, "{}", self.root_node.borrow());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Args;
    use clap::Parser as ClapParser;

    /// Ensure Parser can handle basic Value types
    #[test]
    fn test_simple() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-d"]).build_options());
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
string
---
","
---
true
---
","
---
123456789
---
NULL
"#,
        );

        assert_eq!(format!("{}", p), "string,true,123456789");
    }

    /// Ensure Parser can handle `Value::Sequence`
    #[test]
    fn test_sequence() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-d"]).build_options());
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
- se
- qu
- en
- ce
"#,
        );
        assert_eq!(format!("{}", p), "sequence");

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
- [sub,se]
- qu
- en
- ce
"#,
        );

        assert_eq!(format!("{}", p), "subsequence");

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
- mixed value types
- " "
---
- [here, [is, 1], nested, sequence]
- 54321
---
- true
"#,
        );

        assert_eq!(
            format!("{}", p),
            "mixed value types hereis1nestedsequence54321true"
        );

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
- \{ escaped brace
- \\ escaped backslash
- \\\\ escaped double backslash
- \\{ unclosed variable
"#,
        );

        assert_eq!(
            format!("{}", p),
            r#"{ escaped brace\ escaped backslash\\ escaped double backslash\UNDEFINED"#
        );
    }

    /// Ensure panic on bad YAML
    #[test]
    #[should_panic]
    fn test_bad_yaml() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-d"]).build_options());
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
bad: yaml
error: a: b: c: d: e
"#,
        );
    }

    /// Ensure Parser can handle `Value::Mapping`
    #[test]
    fn test_map() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-d"]).build_options());

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
key: value
"#,
        );
        assert_eq!(format!("{}", p), r#"<key>value</key>"#);

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
key:
  value: data
"#,
        );
        assert_eq!(format!("{}", p), r#"<key><value>data</value></key>"#);

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
key:
  _meta: data
"#,
        );
        assert_eq!(format!("{}", p), r#"<key meta="data"/>"#);

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
key:
  - content
  - _meta: data
  - value: data
  - morecontent
"#,
        );
        assert_eq!(
            format!("{}", p),
            r#"<key meta="data">content<value>data</value>morecontent</key>"#
        );

        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
html:
  head:
    meta:
      _charset: UTF-8
  body:
    p: test
"#,
        );
        assert_eq!(
            format!("{}", p),
            r#"<html><head><meta charset="UTF-8"/></head><body><p>test</p></body></html>"#
        );
    }
}

//! Directives for TaggedValues
//!
//! Includes helper functions to break apart TaggedValue parsing

/* IMPORTS */
use serde::Deserialize;
use serde_yaml::{value::TaggedValue, Deserializer, Value};
use std::{cell::RefCell, fs, path::PathBuf, sync::Arc};

/* LOCAL IMPORTS */
use crate::{debug, error, info, warn, Options, PageNode, Parser};

/* DIRECTIVES */
/// Macro to automate parsing a Value into a boxed str given a target and Value
///
/// $parent: Arc<RefCell<PageNode>>
/// $value: &serde_yaml::Value
#[macro_export]
macro_rules! parse_value {
    ($parent:expr, $value:expr, $dir:expr) => {{
        let child = Arc::new(RefCell::new(PageNode::new($parent.borrow().o.clone())));
        child.borrow_mut().set_parent($parent.clone());
        Parser::add_value(child.clone(), $value, $dir);
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
pub fn if_else(target: Arc<RefCell<PageNode>>, tv: &TaggedValue, dir: Option<PathBuf>) {
    debug!(target.borrow().o, "Evaluating conditional...");
    match &tv.value {
        Value::Sequence(seq) => {
            if seq.len() >= 2 && seq.len() <= 3 {
                let condition = parse_value!(target, &seq[0], dir.clone());
                match &condition[..] {
                    "" => {
                        // exec 'else' block
                        if seq.len() == 3 {
                            Parser::add_value(target.clone(), &seq[2], dir.clone());
                        }
                    }
                    _ => {
                        // exec 'if' block
                        Parser::add_value(target.clone(), &seq[1], dir.clone());
                    }
                }
            }
            return;
        }
        _ => (),
    }
    error!(
        target.borrow().o,
        "Incorrectly formatted conditional: {}",
        value_tostring(&tv.value)
    );
}

/// Get an absolute path to a file that resides (or should reside) in the output directory
///
/// Does the following:
/// - Create a PathBuf to specified file, respecting if it is relative or absolute
/// - Ensure the file resides in the output directory
/// - Throw an error if one of the criteria cannot be satisfied
fn resolve_output_path(
    target: Arc<RefCell<PageNode>>,
    path_str: &str,
    dir: Option<PathBuf>,
) -> Result<PathBuf, Box<str>> {
    if path_str.len() == 0 {
        return Err("Blank path provided!".into());
    }

    let mut path = PathBuf::new();
    if &path_str[..1] == "/" {
        // absolute path (root is output directory)
        path.push(target.borrow().o.output.clone());
        path.push(&path_str[1..]);
    } else {
        // relative path
        path.push(match dir {
            Some(d) => d.to_path_buf(),
            None => target.borrow().o.input.clone(),
        });
        path.push(&path_str[..]);
    }

    // ensure target file is a subnode of the output directory
    if !path.as_path().starts_with(target.borrow().o.output.clone()) {
        return Err(format!(
            "File {f} does not reside in the output directory!",
            f = path.display()
        )
        .into());
    }

    return Ok(path);
}

/// Get an absolute path to a file that resides (or should reside) in the input directory
///
/// Does the following:
/// - Create a PathBuf to specified file, respecting if it is relative or absolute
/// - Ensure the path points to an actually existing file
/// - Ensure the file resides in the input directory
/// - Throw an error if one of the criteria cannot be satisfied
fn resolve_input_path(
    target: Arc<RefCell<PageNode>>,
    path_str: &str,
    dir: Option<PathBuf>,
) -> Result<PathBuf, Box<str>> {
    if path_str.len() == 0 {
        return Err("Blank path provided!".into());
    }

    let mut path = PathBuf::new();
    if &path_str[..1] == "/" {
        // absolute path (root is input directory)
        path.push(target.borrow().o.input.clone());
        path.push(&path_str[1..]);
    } else {
        // relative path
        path.push(match dir {
            Some(d) => d.to_path_buf(),
            None => target.borrow().o.input.clone(),
        });
        path.push(&path_str[..]);
    }

    // canonicalise file path
    let file = match fs::canonicalize(&path) {
        Ok(p) => p,
        Err(e) => {
            return Err(format!(
                "File at '{path}' unable to canonicalise: '{e}'",
                path = &path.display(),
            )
            .into());
        }
    };

    // ensure target file is a subnode of the input directory
    if !file.as_path().starts_with(target.borrow().o.input.clone()) {
        return Err(format!(
            "File {f} does not reside in the input directory!",
            f = file.display()
        )
        .into());
    }

    return Ok(file);
}

/// Blindly copy a file from somewhere in the source directory to somewhere in the destination directory
///
/// File name/extension does not matter, and no checking of file contents is done (blind copy)
/// - File name is always preserved
/// - Relative files are relative to the currently parsed file
/// - Absolute files use the specified source directory as the root folder
/// - Files outside of the source directory and its subdirectories should not be accessed
/// Usage:
/// ```YAML
/// !COPY "relative/file_to_copy"   # destination is relative to current file
/// !COPY "/absolute/file_to_copy"  # destination is absolute using source dir as root
/// ```
pub fn copy(target: Arc<RefCell<PageNode>>, tv: &TaggedValue, dir: Option<PathBuf>) {
    'valid_copy: {
        let s = parse_value!(target, &tv.value, dir.clone());

        // canonicalise paths
        let source = match resolve_input_path(target.clone(), &s, dir.clone()) {
            Ok(s) => s,
            Err(e) => {
                error!(target.borrow().o, "{e}");
                break 'valid_copy;
            }
        };

        let mut dest = target.borrow().o.output.clone();
        dest.push(
            match source.clone().strip_prefix(target.borrow().o.input.clone()) {
                Ok(s) => s,
                Err(e) => panic!("THIS SHOULDN'T EVER HAPPEN BUT IM TOO SCARED TO UNWRAP IT (strip_prefix of input from source failed: {e})"),
            },
        );

        // copy the file
        info!(
            target.borrow().o,
            r#"Copying file "{s}" to "{d}"..."#,
            s = source.display(),
            d = dest.display()
        );

        let mut containing_dir = dest.clone();
        containing_dir.pop();
        match fs::create_dir_all(containing_dir.clone()) {
            Ok(_) => (),
            Err(e) => {
                error!(target.borrow().o, "{e}");
                return; // do not say arguments are invalid if there is just a failure
            }
        }

        match fs::copy(source, dest) {
            Ok(_) => (),
            Err(e) => {
                error!(target.borrow().o, "{e}");
                return;
            }
        };

        return;
    }
    error!(
        target.borrow().o,
        r#"Invalid arguments to !COPY directive: "{}""#,
        value_tostring(&tv.value)
    )
}

/// Include another text or YAML file inside this page
///
/// File name/extension does not matter, it is on the user to ensure it is a properly formatted YAML file (if not using !INCLUDE_RAW)
/// - Relative files are relative to the currently parsed file
/// - Absolute files use the specified source directory as the root folder
/// - Files outside of the source directory and its subdirectories should not be accessed
/// Usage:
/// ```YAML
/// !INCLUDE relative/file_to_include.page
/// !INCLUDE_RAW /absolute/file_to_include.page
/// ```
pub fn include(target: Arc<RefCell<PageNode>>, tv: &TaggedValue, dir: Option<PathBuf>) {
    let s = parse_value!(target, &tv.value, dir.clone());
    let is_raw: bool = tv.tag == "!INCLUDE_RAW";
    info!(target.borrow().o, "Including file {s}...");

    'valid_include: {
        let p = Arc::new(RefCell::new(PageNode::new(target.borrow().o.clone())));
        p.borrow_mut().set_parent(target.clone());

        let file = match resolve_input_path(target.clone(), &s, dir.clone()) {
            Ok(p) => p,
            Err(e) => {
                error!(target.borrow().o, "{e}",);
                break 'valid_include;
            }
        };

        // read the file's YAML into a PageNode
        match fs::read_to_string(file.clone()) {
            Ok(data) => {
                if is_raw {
                    p.borrow_mut().add_content_unparsed(data.into());
                } else {
                    for doc in Deserializer::from_str(data.as_str()) {
                        match Value::deserialize(doc) {
                            Ok(input) => {
                                // swap current file directory
                                let mut new_dir = file.clone();
                                new_dir.pop();
                                Parser::add_value(p.clone(), &input, Some(new_dir))
                            }
                            Err(e) => {
                                panic!("Error while parsing YAML: {e} in {f}", f = file.display())
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!(
                    target.borrow().o,
                    r#"Error reading file "{f}" | {e}"#,
                    f = file.display()
                );
                break 'valid_include;
            }
        }
        target.borrow_mut().add_child(p);

        return;
    }
    error!(
        target.borrow().o,
        r#"Invalid arguments to !INCLUDE directive: "{}""#,
        value_tostring(&tv.value)
    )
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
            let kstr = parse_value!(target, &s[0], None);
            let vstr = parse_value!(target, &s[1], None);
            target.borrow_mut().register_var(kstr, vstr);
        }
    } else {
        error!(
            target.borrow().o,
            r#"Invalid arguments to !DEF directive: "{}""#,
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
pub fn foreach(target: Arc<RefCell<PageNode>>, tv: &TaggedValue, dir: Option<PathBuf>) {
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
                .map(|k| parse_value!(target, k, dir.clone()))
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
                            let vstr = parse_value!(child, v, dir.clone());
                            child
                                .borrow_mut()
                                .register_var(keys[i].clone().into(), vstr.into());
                        });
                        // apply template string
                        Parser::add_value(child, &foreach[1], dir.clone());
                    }
                    _ => (),
                }
            }
            return;
        }
        _ => (),
    }
    let s = value_tostring(&tv.value);
    // if fail
    error!(
        target.borrow().o,
        r#"Invalid arguments to !FOREACH directive: "{}""#,
        if s.len() > 100 {
            format!("{}...", &s[..99])
        } else {
            s
        }
    );
}

/// Convert a serde_yaml::Value to a String
///
/// For use only in debugging or error output, do not include in places where formatting is super important!
fn value_tostring(val: &Value) -> String {
    return match val {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!(r#""{}""#, s.to_string()),
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
                        format!("{}:{},", value_tostring(k), value_tostring(v)),
                    _ => format!("{}:{},", value_tostring(k), value_tostring(v)),
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
    use std::{fs, fs::File, io::Write};

    /// Ensure that combining directives does not cause issues
    #[test]
    fn test_directives_combined() {
        fs::create_dir_all("/tmp/ssgen_test_source_dir_combined").unwrap();
        let o = Arc::new(
            Args::parse_from([
                "",
                "-i",
                "/tmp/ssgen_test_source_dir_combined",
                "-o",
                "/tmp/",
                "-s",
            ])
            .build_options(),
        );

        let mut p = Parser::new(o.clone());
        let mut index = File::create("/tmp/ssgen_test_source_dir_combined/index.page").unwrap();
        index
            .write_all(
                br#"
- !DEF [
    x,
    !FOREACH [
      [y],
      "{y}",
      ["a"],
      ["b"]
    ]
  ]
- p: "{x}"
- !INCLUDE include.block
"#,
            )
            .unwrap();

        let mut include =
            File::create("/tmp/ssgen_test_source_dir_combined/include.block").unwrap();
        include
            .write_all(
                br#"
- p:
    !IF ['{x}', '{x}', "nothing"]
- '{x}': asdf
- !DEF [var2, thisshouldntdoathing]
"#,
            )
            .unwrap();

        p.parse_yaml(
            r#"
!INCLUDE /index.page
"#,
        );

        assert_eq!(format!("{}", p), "<p>ab</p><p>ab</p><ab>asdf</ab>");
    }

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

    /// Ensure Parser can handle !COPY and follow its directives
    #[test]
    fn test_copy() {
        fs::create_dir_all("/tmp/ssgen_test_source_dir_copy/somedir").unwrap();
        fs::create_dir_all("/tmp/ssgen_test_dest_dir_copy").unwrap();
        let o = Arc::new(
            Args::parse_from([
                "",
                "-i",
                "/tmp/ssgen_test_source_dir_copy",
                "-o",
                "/tmp/ssgen_test_dest_dir_copy",
                "-s",
            ])
            .build_options(),
        );

        // copy a file that does not exist
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
!COPY "/somefilethatdoesnotexist"
"#,
        );
        assert_eq!(
            PathBuf::from("/tmp/ssgen_test_dest_dir_copy/somefilethatdoesnotexist")
                .try_exists()
                .unwrap(),
            false
        );

        // copy a file that should not be accessed
        let mut p = Parser::new(o.clone());
        let mut out = File::create("/tmp/inaccessible_file.copy").unwrap();
        out.write_all(b"text").unwrap();

        p.parse_yaml(
            r#"
!COPY "//etc/shadow"
"#,
        );
        assert_eq!(
            PathBuf::from("/tmp/ssgen_test_dest_dir_copy/somefilethatdoesnotexist")
                .try_exists()
                .unwrap(),
            false
        );

        // copy a file that is valid
        let mut p = Parser::new(o.clone());
        let mut out = File::create("/tmp/ssgen_test_source_dir_copy/valid.file").unwrap();
        out.write_all(b"text").unwrap();
        let mut out2 = File::create("/tmp/ssgen_test_source_dir_copy/somedir/valid2.file").unwrap();
        out2.write_all(b"moretext").unwrap();
        p.parse_yaml(
            r#"
- !COPY "/valid.file"
- !COPY "somedir/valid2.file"
"#,
        );

        assert_eq!(
            PathBuf::from("/tmp/ssgen_test_dest_dir_copy/valid.file")
                .try_exists()
                .unwrap(),
            true
        );
        assert_eq!(
            PathBuf::from("/tmp/ssgen_test_dest_dir_copy/somedir/valid2.file")
                .try_exists()
                .unwrap(),
            true
        );

        fs::remove_dir_all("/tmp/ssgen_test_source_dir_copy").unwrap();
        fs::remove_dir_all("/tmp/ssgen_test_dest_dir_copy").unwrap();
    }

    /// Ensure Parser can handle !INCLUDE and follow its directives
    #[test]
    fn test_include() {
        fs::create_dir_all("/tmp/ssgen_test_source_dir_include").unwrap();
        let o = Arc::new(
            Args::parse_from([
                "",
                "-i",
                "/tmp/ssgen_test_source_dir_include",
                "-o",
                "/tmp/",
                "-s",
            ])
            .build_options(),
        );

        // include a file that does not exist
        let mut p = Parser::new(o.clone());
        p.parse_yaml(
            r#"
!INCLUDE /nonexistent_file.page
"#,
        );
        assert_eq!(format!("{}", p), "");

        // include a file that should not be accessed
        let mut p = Parser::new(o.clone());
        let mut out = File::create("/tmp/inaccessible_file.page").unwrap();
        out.write_all(b"p: content").unwrap();

        p.parse_yaml(
            r#"
!INCLUDE /../inaccessible_file.page
"#,
        );
        assert_eq!(format!("{}", p), "");

        // include a file that is valid
        let mut p = Parser::new(o.clone());
        let mut out = File::create("/tmp/ssgen_test_source_dir_include/valid_file.page").unwrap();
        out.write_all(b"p: content").unwrap();
        fs::create_dir_all("/tmp/ssgen_test_source_dir_include/inc").unwrap();
        let mut out2 =
            File::create("/tmp/ssgen_test_source_dir_include/inc/another_valid_file.page").unwrap();
        out2.write_all(b"- !INCLUDE /valid_file.page\n- !INCLUDE ../valid_file.page")
            .unwrap();

        p.parse_yaml(
            r#"
- !INCLUDE
- !INCLUDE /valid_file.page
- sep
- !INCLUDE valid_file.page
- !INCLUDE inc/another_valid_file.page
- !INCLUDE_RAW valid_file.page
"#,
        );

        assert_eq!(
            format!("{}", p),
            "<p>content</p>sep<p>content</p><p>content</p><p>content</p>p: content"
        );

        fs::remove_dir_all("/tmp/ssgen_test_source_dir_include").unwrap();
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
        assert_eq!(
            value_tostring(&Value::String("asdf".to_string())),
            r#""asdf""#
        );

        // mapping
        let m: Value = serde_yaml::from_str(r#"{a: b, 1: cdefg, h: [i, j, k]}"#).unwrap();
        assert_eq!(
            value_tostring(&m),
            r#"{"a":"b",1:"cdefg","h":["i","j","k",],}"#
        );

        // tagged
        let t: Value = serde_yaml::from_str("!TAG value").unwrap();
        assert_eq!(value_tostring(&t), r#"!TAG "value""#);

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
            r#"[NULL,123,"abc",true,[NULL,123,"abc",true,],{"a":"b",1:"cdefg","h":["i","j","k",],},!TAG "value",]"#
        );
    }
}

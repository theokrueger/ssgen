//! A PageNode struct represents one HTML node
//!
//! PageNode can be combined into a tree that can represent a full HTML webpage
//! ```
//! let o = Arc::new(Args::parse().build_options());
//!
//! let mut parent = PageNode::new(o.clone());
//! parent.set_name("HTMLNode".into());
//! parent.add_metadata(("class".into(), "SomeClass".into()));
//! let mut child = PageNode::new(o.clone());
//! child.set_content("{MyContent}".into());
//!
//! assert_eq!(
//!     format!("{parent}"),
//!     r#"<HTMLNode class="SomeClass">Content</HTMLNode>"#
//! );
//! ```

/* IMPORTS */
use std::{
    cell::RefCell,
    collections::{HashMap, LinkedList},
    fmt,
    sync::Arc,
};

/* LOCAL IMPORTS */
use crate::{debug, error, info, warn, Options};

/* PAGENODE */
/// A PageNode is a node in a tree, where the tree can be resolved into a complete webpage
pub struct PageNode {
    /// Name of the node
    name: Box<str>,

    /// Metadata for node, i.e. class="SomeClass"
    metadata: LinkedList<(Box<str>, Box<str>)>,

    /// Text content of node. This always be empty unless there is no name and no children.
    content: String,

    /// Children nodes of this page node
    children: LinkedList<Arc<RefCell<PageNode>>>,

    /// parent node of this page node
    parent: Option<Arc<RefCell<PageNode>>>,

    /// Mapping containing variables inside the current scope
    vars: HashMap<Box<str>, Box<str>>,

    /// Program-wide options and logger, see args::Options for more.
    pub o: Arc<Options>,
}

impl PageNode {
    /// Create a new, empty PageNode with no parent
    pub fn new(o: Arc<Options>) -> Self {
        return Self {
            name: "".into(),
            metadata: LinkedList::new(),
            children: LinkedList::new(),
            content: "".into(),
            parent: None,
            vars: HashMap::new(),
            o: o,
        };
    }

    /// Register a variable into this node
    pub fn register_var(&mut self, k: Box<str>, v: Box<str>) {
        let key = self.parse_string(k);
        let val = self.parse_string(v);
        debug!(self.o, "Registering variable {key}");
        self.vars.insert(key, val);
    }

    /// Get the value of a variable from this node or its parents
    ///
    /// Search the current node first, then sequentially search parent nodes until variable is found.
    /// If variable does not exist in the node tree, return a placeholder
    pub fn get_var(&self, k: Box<str>) -> Box<str> {
        // search self
        match self.vars.get(&k) {
            Some(v) => return v.clone(),
            None => (),
        };
        // search parent
        match &self.parent {
            Some(p) => return p.borrow().get_var(k),
            None => {
                warn!(self.o, "Undefined variable {k}");
                return "UNDEFINED".to_string().into_boxed_str();
            }
        }
    }

    /// Add a new child to the end of children
    pub fn add_child(&mut self, child: Arc<RefCell<PageNode>>) {
        self.children.push_back(child);
    }

    /// Add some new metadata to the node
    pub fn add_metadata(&mut self, kvpair: (Box<str>, Box<str>)) {
        self.metadata.push_back(kvpair);
    }

    /// Set content of node, taking ownership of passed text
    pub fn add_content(&mut self, s: Box<str>) {
        self.content += &self.parse_string(s.into());
    }

    /// Set parent of node, taking ownership of passed Arc
    pub fn set_parent(&mut self, p: Arc<RefCell<PageNode>>) {
        self.parent = Some(p.clone());
    }

    /// Set name of node, taking ownership of passed text
    pub fn set_name(&mut self, s: Box<str>) {
        self.name = self.parse_string(s);
    }

    /// Formats strings according to settings
    ///
    /// Does the following:
    /// - Replaces variables marked as {var} with their values from self.vars
    ///   - Variables can be inserted anywhere users can define text
    ///   - This means that regiestering a variable k='{var}' v='value' is 'somename: value' where 'var' is defined as 'somename'
    ///   - Setting content to '{{x}}' is also allowed and will evaluate (where 'x' = 'var', 'var' = '2') to '${var}' then to 'two'
    ///   - Variables can be escaped with '\\{' (literal backslash)
    pub fn parse_string(&self, s: Box<str>) -> Box<str> {
        const BUFSIZE: usize = 250; // should be divisible by 10
        let mut builder = String::with_capacity(BUFSIZE);

        // iterate over chars
        let mut prev: char = ' ';
        let mut iter = s.chars().peekable();
        let mut c: char;
        loop {
            match iter.next() {
                Some(x) => c = x,
                None => break,
            }
            match c {
                // potentially start variable
                '{' => {
                    if prev == '\\' {
                        // brace is escaped, add as normal
                        builder.push(c)
                    } else {
                        // start of the variable!!! :D
                        let mut brace_depth: u8 = 0;
                        let mut var_builder = String::with_capacity(BUFSIZE / 10);
                        loop {
                            match iter.next() {
                                Some(x) => c = x,
                                None => {
                                    error!(
                                        self.o,
                                        "Unclosed variable delimiter in {}...",
                                        if s.len() > 40 { &s[0..39] } else { &s }
                                    );
                                    break;
                                }
                            }
                            match c {
                                // start of sub-variable
                                '{' => {
                                    var_builder.push(c);
                                    brace_depth += 1;
                                }
                                // end of variable or sub-variable
                                '}' => {
                                    if brace_depth == 0 {
                                        break;
                                    }
                                    brace_depth -= 1;
                                    var_builder.push(c);
                                }
                                // other
                                _ => var_builder.push(c),
                            }
                        }
                        // variable built, get var now
                        var_builder = self.parse_string(var_builder.into()).into();
                        builder += &self.get_var(var_builder.into());
                    }
                }
                // escape sequence
                '\\' => {
                    if prev == '\\' {
                        builder.push(c);
                        c = ' ';
                    }
                }
                // not the start of anything
                _ => {
                    builder.push(c);
                }
            }
            prev = c
        }
        return builder.into_boxed_str();
    }
}

impl fmt::Display for PageNode {
    /// Resolve a PageNode and all its children into text
    ///
    /// Has the following cases for formatting:
    /// - No name and no children: `"{content}"` (ignores metadata)
    /// - No name and children: `"{content{{children}"` (ignores metadata)
    /// - Name and no children: `"{content}<{name} {metadata}/>"`
    /// - Name and children: `"<{name} {metadata}>{content}{children}</{name}>"`
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let case = (self.children.len() != 0 || self.content.len() != 0) as u8
            + (self.name.len() != 0) as u8 * 2;
        match case {
            // no name, children(?)
            0 | 1 => {
                write!(f, "{}", self.content)?;
                for x in self.children.iter() {
                    write!(f, "{}", x.borrow())?;
                }
            }
            // name, no children
            2 => {
                write!(
                    f,
                    "<{name}{metadata}/>",
                    name = self.name,
                    metadata = self
                        .metadata
                        .iter()
                        .map(|(k, v)| format!(r#" {k}="{v}""#))
                        .collect::<String>()
                )?;
            }
            //name, children or content
            _ => {
                write!(
                    f,
                    "<{name}{metadata}>",
                    name = self.name,
                    metadata = self
                        .metadata
                        .iter()
                        .map(|(k, v)| format!(r#" {k}="{v}""#))
                        .collect::<String>()
                )?;
                write!(f, "{}", self.content)?;
                for x in self.children.iter() {
                    write!(f, "{}", x.borrow())?;
                }
                write!(f, "</{name}>", name = self.name)?;
            }
        }

        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Args;
    use clap::Parser;

    /// empsure a pagenode can be created and its contents can be accessed as needed
    #[test]
    fn test_empty() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-d"]).build_options());
        let p = PageNode::new(o.clone());
        info!(o, "test log");
        assert_eq!(format!("{}", p), "");
    }

    /// Test the four different display formats for a PageNode
    #[test]
    fn test_pagenode_display() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-d"]).build_options());

        let mut noname_nochild = PageNode::new(o.clone());
        noname_nochild.add_content("some content".into());
        assert_eq!(format!("{}", noname_nochild), "some content");

        let mut noname_child = PageNode::new(o.clone());
        noname_child.add_child(Arc::new(RefCell::new(noname_nochild)));
        assert_eq!(format!("{}", noname_child), "some content");

        let mut name_nochild = PageNode::new(o.clone());
        name_nochild.set_name("somename".into());
        name_nochild.add_metadata(("class".into(), "someclass".into()));
        assert_eq!(
            format!("{}", name_nochild),
            r#"<somename class="someclass"/>"#
        );

        let mut name_child = noname_child;
        name_child.set_name("somename".into());
        name_child.add_metadata(("class".into(), "someclass".into()));
        name_child.add_metadata(("style".into(), "somestyle".into()));
        assert_eq!(
            format!("{}", name_child),
            r#"<somename class="someclass" style="somestyle">some content</somename>"#
        );
    }

    /// Test string parsing
    #[test]
    fn test_parse_string() {
        let o = Arc::new(Args::parse_from(["", "-i", "./", "-o", "/tmp/", "-d"]).build_options());

        let mut node = PageNode::new(o.clone());
        node.register_var("x".into(), "69".into());
        node.add_content("The value of x is {x}".into());
        assert_eq!(format!("{}", node), "The value of x is 69");

        let mut node = PageNode::new(o.clone());
        node.register_var("x".into(), "69".into());
        node.register_var("{x}".into(), "funny number".into());
        node.add_content("The value of 69 is {69}".into());
        assert_eq!(format!("{}", node), "The value of 69 is funny number");

        let mut node = PageNode::new(o.clone());
        node.register_var("x".into(), "69".into());
        node.register_var("{x}".into(), "funny number".into());
        node.register_var("{69}".into(), "funny number {x}".into());
        node.add_content("The value of funny number is {funny number}".into());
        assert_eq!(
            format!("{}", node),
            "The value of funny number is funny number 69"
        );

        let mut node = PageNode::new(o.clone());
        node.register_var("x".into(), "y".into());
        node.register_var("y".into(), "z".into());
        node.add_content("{{x}}".into());
        assert_eq!(format!("{}", node), "z");

        let mut node = PageNode::new(o.clone());
        node.add_content("\\{novar\\}".into());
        assert_eq!(format!("{}", node), "{novar}");

        let mut node = PageNode::new(o.clone());
        node.add_content("{undefined variable}".into());
        assert_eq!(format!("{}", node), "UNDEFINED");

        let node = Arc::new(RefCell::new(PageNode::new(o.clone())));
        node.borrow_mut().register_var("x".into(), "y".into());
        node.borrow_mut().set_name("name".into());
        let child = Arc::new(RefCell::new(PageNode::new(o.clone())));
        node.borrow_mut().add_child(child.clone());
        child.borrow_mut().set_parent(node.clone());
        child.borrow_mut().add_content("{x}".into());
        assert_eq!(format!("{}", node.borrow()), "<name>y</name>");
    }
}

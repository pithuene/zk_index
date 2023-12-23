//! An extension for markdown-it that parses wikilinks.
//! A wikilink is a link that looks like this: `[[link]]`.

use markdown_it::{
    parser::inline::{InlineRule, InlineState},
    MarkdownIt, Node, NodeValue, Renderer,
};
use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct Wikilink {
    pub target: String,
}

impl NodeValue for Wikilink {
    fn render(&self, _: &Node, fmt: &mut dyn Renderer) {
        fmt.text_raw(&self.target);
    }
}

pub fn add(md: &mut MarkdownIt) {
    md.inline.add_rule::<WikilinkScanner>();
}

// Create a regex to match the wikilink and capture its target.
pub static WIKILINK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[\[[^\]]+\]\]").expect("Failed to compile WIKILINK_RE regex."));

#[doc(hidden)]
pub struct WikilinkScanner;
impl InlineRule for WikilinkScanner {
    const MARKER: char = '[';
    fn run(state: &mut InlineState) -> Option<(Node, usize)> {
        let capture: Option<&str> = WIKILINK_RE
            .captures(&state.src[state.pos..state.pos_max])?
            .get(0)
            .map(|m| m.as_str());

        match capture {
            Some(capture) => {
                // The capture includes the brackets, so we need to remove them.
                let target = &capture[2..capture.len() - 2];

                let node = Node::new(Wikilink {
                    target: target.to_string(),
                });
                Some((node, target.len()))
            }
            None => None,
        }
    }
}

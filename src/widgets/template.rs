//! Text templates: `{field}` inserts a reading's field, `[ ... ]` is an
//! optional group dropped when any field inside is missing, and `\` escapes
//! the next character.

use crate::metrics::Reading;
use crate::style::{Piece, Role};

#[derive(Debug, PartialEq)]
enum Node {
    Lit(String),
    Field(String),
    Group(Vec<Node>),
}

fn parse(template: &str) -> Vec<Node> {
    fn parse_until(chars: &mut std::iter::Peekable<std::str::Chars>, in_group: bool) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut lit = String::new();
        let flush = |lit: &mut String, nodes: &mut Vec<Node>| {
            if !lit.is_empty() {
                nodes.push(Node::Lit(std::mem::take(lit)));
            }
        };
        while let Some(c) = chars.next() {
            match c {
                '\\' => lit.extend(chars.next()),
                '{' => {
                    flush(&mut lit, &mut nodes);
                    let name: String = chars.by_ref().take_while(|c| *c != '}').collect();
                    nodes.push(Node::Field(name.trim().to_string()));
                }
                '[' => {
                    flush(&mut lit, &mut nodes);
                    nodes.push(Node::Group(parse_until(chars, true)));
                }
                ']' if in_group => break,
                c => lit.push(c),
            }
        }
        flush(&mut lit, &mut nodes);
        nodes
    }
    parse_until(&mut template.chars().peekable(), false)
}

fn complete(nodes: &[Node], reading: &Reading) -> bool {
    nodes.iter().all(|n| match n {
        Node::Lit(_) => true,
        Node::Field(name) => reading.get(name).is_some(),
        Node::Group(_) => true,
    })
}

fn emit(nodes: &[Node], reading: &Reading, out: &mut Vec<Piece>) {
    for node in nodes {
        match node {
            Node::Lit(text) => out.push(Piece::new(text.clone(), Role::Text)),
            Node::Field(name) => {
                if let Some(f) = reading.get(name) {
                    out.push(Piece::new(f.text.clone(), f.role.unwrap_or(Role::Text)));
                }
            }
            Node::Group(inner) => {
                if complete(inner, reading) {
                    emit(inner, reading, out);
                }
            }
        }
    }
}

/// Render `template` for `reading`; empty when nothing visible remains.
pub fn render(template: &str, reading: &Reading) -> Vec<Piece> {
    // The top level behaves like a group: a missing top-level field means the
    // metric lacks its main value, so nothing is shown.
    let nodes = parse(template);
    if !complete(&nodes, reading) {
        return Vec::new();
    }
    let mut out = Vec::new();
    emit(&nodes, reading, &mut out);
    if out.iter().all(|p| p.text.trim().is_empty()) {
        out.clear();
    }
    merge(out)
}

/// Join neighbouring pieces with the same role.
pub fn merge(pieces: Vec<Piece>) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::with_capacity(pieces.len());
    for p in pieces {
        match out.last_mut() {
            Some(last) if last.role == p.role && last.link == p.link => last.text.push_str(&p.text),
            _ => out.push(p),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(pieces: &[Piece]) -> String {
        pieces.iter().map(|p| p.text.as_str()).collect()
    }

    #[test]
    fn fills_fields_and_drops_incomplete_groups() {
        let r = Reading::default().set("pct", "62").set("tokens", "124k");
        assert_eq!(text(&render("{pct}%[ · {tokens}]", &r)), "62% · 124k");
        let r = Reading::default().set("pct", "62");
        assert_eq!(text(&render("{pct}%[ · {tokens}]", &r)), "62%");
    }

    #[test]
    fn keeps_field_roles() {
        let r = Reading::default()
            .set_role("added", "+3", Role::Good)
            .set_role("removed", "-1", Role::Bad);
        let pieces = render("[{added}][ {removed}]", &r);
        assert_eq!(pieces[0], Piece::new("+3", Role::Good));
        assert_eq!(pieces[2], Piece::new("-1", Role::Bad));
    }

    #[test]
    fn empty_when_nothing_visible() {
        let r = Reading::default();
        assert!(render("[{left}][{cold}]", &r).is_empty());
        assert!(render("{missing}", &r).is_empty());
        assert!(render("{pct}%", &r).is_empty());
    }

    #[test]
    fn supports_escapes_and_literal_templates() {
        let r = Reading::default().set("n", "3");
        assert_eq!(text(&render("\\[{n}\\]", &r)), "[3]");
        assert_eq!(text(&render("thinking", &r)), "thinking");
    }
}

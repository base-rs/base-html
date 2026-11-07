// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::confined::confined;
use crate::template_node::TemplateNode;
use std::collections::HashSet;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, ExprCall, Ident, LitStr, Pat, Result, Token, braced, parenthesized};

pub(crate) struct Template {
    pub destination: Expr,
    pub nodes: Vec<TemplateNode>,
}

impl Parse for Template {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let destination = input.parse()?;
        input.parse::<Token![;]>()?;
        Ok(Self {
            destination,
            nodes: parse_nodes(input)?,
        })
    }
}

fn parse_nodes(input: ParseStream<'_>) -> Result<Vec<TemplateNode>> {
    let mut nodes = Vec::new();
    while !input.is_empty() {
        if input.peek(LitStr) {
            let text: LitStr = input.parse().expect("peek established a string literal");
            nodes.push(TemplateNode::Literal(escape(&text.value())));
        } else if input.peek(syn::token::Paren) {
            nodes.push(TemplateNode::Text(parse_expression(input)?));
        } else if input.peek(Token![@]) {
            parse_directive(input, &mut nodes)?;
        } else {
            parse_element(input, &mut nodes)?;
        }
    }
    Ok(nodes)
}

fn parse_expression(input: ParseStream<'_>) -> Result<Expr> {
    let content;
    parenthesized!(content in input);
    let expression = content.parse()?;
    if !content.is_empty() {
        return Err(content.error("expected one Rust expression"));
    }
    confined(&expression)?;
    Ok(expression)
}

fn parse_body(input: ParseStream<'_>) -> Result<Vec<TemplateNode>> {
    let content;
    braced!(content in input);
    parse_nodes(&content)
}

fn parse_directive(input: ParseStream<'_>, nodes: &mut Vec<TemplateNode>) -> Result<()> {
    input
        .parse::<Token![@]>()
        .expect("caller peeked an @ token");
    let directive = input.call(Ident::parse_any)?;
    match directive.to_string().as_str() {
        "for" => {
            let pattern = input.call(Pat::parse_single)?;
            input.parse::<Token![in]>()?;
            let expression = input.call(Expr::parse_without_eager_brace)?;
            confined(&expression)?;
            nodes.push(TemplateNode::For(pattern, expression, parse_body(input)?));
        }
        "if" => nodes.push(parse_if(input)?),
        "let" => {
            let pattern = input.call(Pat::parse_single)?;
            input.parse::<Token![=]>()?;
            let expression = input.parse()?;
            confined(&expression)?;
            input.parse::<Token![;]>()?;
            nodes.push(TemplateNode::Let(pattern, expression));
        }
        "render" => {
            let call: ExprCall = input.parse()?;
            confined(&Expr::Call(call.clone()))?;
            input.parse::<Token![;]>()?;
            nodes.push(TemplateNode::Render(call));
        }
        "raw" => {
            let expression = parse_expression(input)?;
            input.parse::<Token![;]>()?;
            nodes.push(TemplateNode::Trusted(expression));
        }
        "doctype" => {
            input.parse::<Token![;]>()?;
            nodes.push(TemplateNode::Literal("<!DOCTYPE html>".into()));
        }
        "flush" => {
            input.parse::<Token![;]>()?;
            nodes.push(TemplateNode::Flush);
        }
        _ => {
            return Err(syn::Error::new(
                directive.span(),
                "expected @for, @if, @let, @render, @raw, @doctype, or @flush",
            ));
        }
    }
    Ok(())
}

fn parse_if(input: ParseStream<'_>) -> Result<TemplateNode> {
    let condition = input.call(Expr::parse_without_eager_brace)?;
    confined(&condition)?;
    let body = parse_body(input)?;
    let otherwise = if input.peek(Token![@]) && input.peek2(Token![else]) {
        input.parse::<Token![@]>().expect("peek established @else");
        input
            .parse::<Token![else]>()
            .expect("peek established @else");
        if input.peek(Token![if]) {
            input.parse::<Token![if]>().expect("peek established if");
            vec![parse_if(input)?]
        } else {
            parse_body(input)?
        }
    } else {
        Vec::new()
    };
    Ok(TemplateNode::If(condition, body, otherwise))
}

fn parse_name(input: ParseStream<'_>) -> Result<(String, proc_macro2::Span)> {
    let first = input.call(Ident::parse_any)?;
    let mut name = first.to_string();
    while input.peek(Token![-]) || input.peek(Token![:]) {
        let separator: proc_macro2::Punct =
            input.parse().expect("peek established name punctuation");
        let next = input.call(Ident::parse_any)?;
        name.push(separator.as_char());
        name.push_str(&next.to_string());
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"_-:".contains(&byte))
    {
        return Err(syn::Error::new(
            first.span(),
            "HTML names must use ASCII letters, digits, underscores, hyphens, or colons",
        ));
    }
    Ok((name, first.span()))
}

fn parse_element(input: ParseStream<'_>, nodes: &mut Vec<TemplateNode>) -> Result<()> {
    let (name, span) = parse_name(input)?;
    let lower = name.to_ascii_lowercase();
    if lower == "plaintext" {
        return Err(syn::Error::new(
            span,
            "plaintext cannot be represented as a balanced HTML element",
        ));
    }
    nodes.push(TemplateNode::Literal(format!("<{name}")));
    let mut attributes = HashSet::new();
    while !input.peek(syn::token::Brace) && !input.peek(Token![;]) {
        let (attribute, span) = parse_name(input)?;
        let lower = attribute.to_ascii_lowercase();
        if !attributes.insert(lower.clone()) {
            return Err(syn::Error::new(span, "duplicate HTML attribute"));
        }
        input.parse::<Token![=]>()?;
        nodes.push(TemplateNode::Literal(format!(" {attribute}=\"")));
        if input.peek(LitStr) {
            let value: LitStr = input.parse().expect("peek established a string literal");
            nodes.push(TemplateNode::Literal(escape(&value.value())));
        } else {
            if lower.starts_with("on") || lower == "style" || lower == "srcdoc" {
                return Err(syn::Error::new(
                    span,
                    "dynamic event-handler, style, and srcdoc attributes are not supported",
                ));
            }
            nodes.push(TemplateNode::Text(parse_expression(input)?));
        }
        nodes.push(TemplateNode::Literal("\"".into()));
    }
    nodes.push(TemplateNode::Literal(">".into()));
    let is_void = matches!(
        lower.as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    );
    if is_void {
        input.parse::<Token![;]>().map_err(|_| {
            syn::Error::new(
                span,
                "void elements must end with `;` and cannot have children",
            )
        })?;
        return Ok(());
    }
    if matches!(
        lower.as_str(),
        "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "noscript"
    ) {
        let content;
        braced!(content in input);
        while !content.is_empty() {
            if !content.peek(LitStr) {
                return Err(content.error("this element only supports static string literals; dynamic HTML escaping is not safe in this context"));
            }
            let text: LitStr = content.parse().expect("peek established a string literal");
            nodes.push(TemplateNode::Literal(text.value()));
        }
    } else {
        nodes.extend(parse_body(input)?);
    }
    nodes.push(TemplateNode::Literal(format!("</{name}>")));
    Ok(())
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

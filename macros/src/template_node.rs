// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Expr, ExprCall, Pat};

pub(crate) enum TemplateNode {
    Literal(String),
    Text(Expr),
    Trusted(Expr),
    For(Pat, Expr, Vec<Self>),
    If(Expr, Vec<Self>, Vec<Self>),
    Let(Pat, Expr),
    Render(ExprCall),
    Flush,
}

pub(crate) fn compile(nodes: &[TemplateNode], writer: &Ident) -> TokenStream {
    let mut output = TokenStream::new();
    let mut literal = String::new();
    for node in nodes {
        let tokens = match node {
            TemplateNode::Literal(value) => {
                literal.push_str(value);
                continue;
            }
            TemplateNode::Text(expression) => quote! {
                #writer.write_text(&(#expression)).await?;
            },
            TemplateNode::Trusted(expression) => quote! {
                #writer.write_trusted(#expression).await?;
            },
            TemplateNode::For(pattern, expression, body) => {
                let body = compile(body, writer);
                quote! { for #pattern in #expression { #body } }
            }
            TemplateNode::If(condition, body, otherwise) => {
                let body = compile(body, writer);
                let otherwise = compile(otherwise, writer);
                quote! { if #condition { #body } else { #otherwise } }
            }
            TemplateNode::Let(pattern, expression) => quote! {
                let #pattern = #expression;
            },
            TemplateNode::Render(call) => {
                let function = &call.func;
                let arguments = &call.args;
                quote! { #function(#writer, #arguments).await?; }
            }
            TemplateNode::Flush => quote! { #writer.flush().await?; },
        };
        flush(&mut literal, &mut output, writer);
        output.extend(tokens);
    }
    flush(&mut literal, &mut output, writer);
    output
}

fn flush(literal: &mut String, output: &mut TokenStream, writer: &Ident) {
    if !literal.is_empty() {
        output.extend(quote! { #writer.write_literal(#literal).await?; });
        literal.clear();
    }
}

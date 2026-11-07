// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Template expressions must finish where they start.
//!
//! Each expression is spliced into the generated rendering code. A `return`, or a `break` or `continue` that leaves the expression, would skip the template's remaining static markup, such as a closing quote or tag, and the following escaped text could then be parsed as markup or attributes. `?` remains allowed: it ends rendering with an error, which callers already treat as an incomplete document.
use syn::visit::{self, Visit};
use syn::{Expr, Label, Lifetime, Result};

/// Reject control flow that leaves `expression`.
pub(crate) fn confined(expression: &Expr) -> Result<()> {
    let mut check = Check::default();
    check.visit_expr(expression);
    check.error.map_or(Ok(()), Err)
}

#[derive(Default)]
struct Check {
    /// Unlabeled loops entered within the expression.
    loops: usize,
    /// Labels of loops and blocks entered within the expression.
    labels: Vec<String>,
    error: Option<syn::Error>,
}

impl Check {
    fn reject(&mut self, span: proc_macro2::Span, keyword: &str) {
        self.error.get_or_insert_with(|| {
            syn::Error::new(
                span,
                format!("`{keyword}` cannot leave a template expression; it would skip the template's remaining markup"),
            )
        });
    }

    fn scan(&mut self, tokens: proc_macro2::TokenStream) {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Ident(ident)
                    if ident == "return" || ident == "break" || ident == "continue" =>
                {
                    self.reject(ident.span(), &ident.to_string());
                }
                proc_macro2::TokenTree::Group(group) => self.scan(group.stream()),
                _ => {}
            }
        }
    }

    fn escapes(&self, label: Option<&Lifetime>) -> bool {
        match label {
            Some(label) => !self.labels.contains(&label.ident.to_string()),
            None => self.loops == 0,
        }
    }

    /// Visit a loop body with its label, and unlabeled `break` and `continue`, bound to it.
    fn enter(&mut self, label: Option<&Label>, body: impl FnOnce(&mut Self)) {
        self.loops += 1;
        self.labels
            .extend(label.map(|label| label.name.ident.to_string()));
        body(self);
        self.labels
            .truncate(self.labels.len() - usize::from(label.is_some()));
        self.loops -= 1;
    }
}

impl<'ast> Visit<'ast> for Check {
    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        self.reject(node.return_token.span, "return");
    }

    fn visit_expr_break(&mut self, node: &'ast syn::ExprBreak) {
        if self.escapes(node.label.as_ref()) {
            self.reject(node.break_token.span, "break");
        }
        visit::visit_expr_break(self, node);
    }

    fn visit_expr_continue(&mut self, node: &'ast syn::ExprContinue) {
        if self.escapes(node.label.as_ref()) {
            self.reject(node.continue_token.span, "continue");
        }
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.enter(node.label.as_ref(), |check| check.visit_block(&node.body));
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        // Rust rejects unlabeled `break` and `continue` in the condition (E0590); checking it outside the loop is at least as strict.
        self.visit_expr(&node.cond);
        self.enter(node.label.as_ref(), |check| check.visit_block(&node.body));
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        // The iterator is evaluated before the loop starts, so it is not inside it.
        self.visit_expr(&node.expr);
        self.enter(node.label.as_ref(), |check| check.visit_block(&node.body));
    }

    fn visit_expr_block(&mut self, node: &'ast syn::ExprBlock) {
        // Only a labeled `break` can target a block.
        let label = node
            .label
            .as_ref()
            .map(|label| label.name.ident.to_string());
        let labeled = label.is_some();
        self.labels.extend(label);
        visit::visit_expr_block(self, node);
        self.labels
            .truncate(self.labels.len() - usize::from(labeled));
    }

    // Macro arguments are expanded later and cannot be checked as code, so any of these keywords in them is rejected, even inside a loop or closure there.
    // SECURITY: Control flow produced by a macro's own expansion, including a `macro_rules!` defined inside the expression, cannot be checked here. This is not a weakness: template source is trusted application code, and the check guards against mistakes, not attackers. The limitation is documented in context/templates.md.
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        self.scan(node.tokens.clone());
    }

    // Closures, async blocks, and items are separate bodies: their control flow cannot leave the expression.
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}

    fn visit_expr_async(&mut self, _: &'ast syn::ExprAsync) {}

    fn visit_item(&mut self, _: &'ast syn::Item) {}
}

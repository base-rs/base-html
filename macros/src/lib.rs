// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The compile-time implementation of `base_html::html`.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod confined;
mod template;
mod template_node;

#[cfg(test)]
mod tests;

/// Compile a structurally balanced HTML template into escaped writes.
///
/// The first argument is an `HtmlWriter`, followed by `;` and template content. The macro returns a borrowing future producing `std::io::Result<()>`; await it to render. It supports native awaited expressions, async component calls, and explicit `@flush;` boundaries. See the `base-html` crate's syntax guide for the full language.
#[proc_macro]
pub fn html(input: TokenStream) -> TokenStream {
    let template = parse_macro_input!(input as template::Template);
    let destination = template.destination;
    let writer = proc_macro2::Ident::new("__base_html_writer", proc_macro2::Span::mixed_site());
    let body = template_node::compile(&template.nodes, &writer);
    quote! {{
        async {
            let #writer = (#destination).by_ref();
            #body
            ::std::result::Result::<(), ::std::io::Error>::Ok(())
        }
    }}
    .into()
}

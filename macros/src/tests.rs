// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{template::Template, template_node};
use proc_macro2::{Ident, Span};

#[test]
fn rejects_malformed_rust_and_template_boundaries() {
    for source in [
        "",
        "out",
        "out; (1 2)",
        "out; ()",
        "out; @",
        "out; @for in values {}",
        "out; @for value values {}",
        "out; @for value in + {}",
        "out; @for value in values ;",
        "out; @for value in values { () }",
        "out; @if + {}",
        "out; @if true ;",
        "out; @if true { () }",
        "out; @if true {} @else if + {}",
        "out; @if true {} @else ;",
        "out; @let = 1;",
        "out; @let value 1;",
        "out; @let value = +;",
        "out; @let value = 1",
        "out; @render 1;",
        "out; @render component()",
        "out; @raw(value)",
        "out; @raw() ;",
        "out; @doctype",
        "out; @flush",
        "out; @unknown;",
        "out; 42",
        "out; custom- {}",
        "out; custom: {}",
        "out; 日本語 {}",
        "out; plaintext {}",
        "out; p 42 {}",
        "out; p class {}",
        "out; p class=42 {}",
        "out; p class=() {}",
        "out; p class=\"one\" CLASS=\"two\" {}",
        "out; p onclick=(value) {}",
        "out; p STYLE=(value) {}",
        "out; p srcdoc=(value) {}",
        "out; input {}",
        "out; p;",
        "out; p { () }",
        "out; script;",
        "out; script { (value) }",
        "out; style { @render component(); }",
        "out; xmp { @for value in values { (value) } }",
        "out; iframe { @raw(trusted); }",
        "out; @for value in values { li title=({ if skip { continue; } value }) {} }",
        "out; @for value in values { (if skip { break } else { value }) }",
        "out; p title=({ if done { return Ok(()); } value }) {}",
        "out; @raw({ return Ok(()) });",
        "out; @let value = loop { return Ok(()) };",
        "out; @if { continue } {}",
        "out; @for value in { break } {}",
        "out; @render component(return Ok(()));",
        "out; ('outer: loop { break 'inner; })",
        "out; (for item in { continue } { item })",
        "out; (while { break } {})",
        "out; (loop { (|| 1)(); return Ok(()) })",
        "out; @for value in values { li title=(format!(\"{}\", if skip { continue } else { value })) {} }",
        "out; (vec![{ return Ok(()) }])",
    ] {
        assert!(
            syn::parse_str::<Template>(source).is_err(),
            "accepted invalid template: {source}"
        );
    }
}

#[test]
fn preserves_static_context_and_supports_namespaces() {
    let template = syn::parse_str::<Template>(
        "out; svg xml:lang=\"en\" { custom-element data-label=\"ok\" {} } SCRIPT {} noscript { \"<p>fallback</p>\" }",
    ).unwrap();
    let output = template_node::compile(&template.nodes, &Ident::new("writer", Span::call_site()))
        .to_string();
    assert!(output.contains("xml:lang"));
    assert!(output.contains("<noscript><p>fallback</p></noscript>"));
    assert_eq!(output.matches("write_literal").count(), 1);
}

#[test]
fn generates_shared_writer_calls_and_preserves_control_flow() {
    let template = syn::parse_str::<Template>(
        r#"
        out;
        @let values = [1, 2];
        div {
            @for value in values {
                @if value == 1 { (value) }
                @else if value == 2 { @render component(value); }
                @else { @raw(trusted); }
            }
        }
    "#,
    )
    .unwrap();
    let output = template_node::compile(&template.nodes, &Ident::new("writer", Span::call_site()))
        .to_string();
    assert!(output.contains("component (writer , value)"));
    assert!(output.contains("write_trusted"));
    assert!(output.contains("write_text"));
    assert!(!output.contains("String"));
}

#[test]
fn flush_separates_static_fragments_and_all_writes_are_awaited() {
    let template = syn::parse_str::<Template>("out; p { \"prefix\" @flush; \"suffix\" }").unwrap();
    let output = template_node::compile(&template.nodes, &Ident::new("writer", Span::call_site()))
        .to_string();
    assert_eq!(output.matches("write_literal").count(), 2);
    assert_eq!(output.matches("await").count(), 3);
    assert!(output.find("prefix").unwrap() < output.find("flush").unwrap());
    assert!(output.find("flush").unwrap() < output.find("suffix").unwrap());
}

#[test]
fn control_flow_that_stays_inside_an_expression_is_allowed() {
    for source in [
        "out; (values.iter().map(|value| { if *value > 1 { return 0; } *value }).sum::<i32>())",
        "out; (loop { break 1; })",
        "out; ('outer: loop { loop { break 'outer 2; } })",
        "out; ('block: { if ready { break 'block 3; } 4 })",
        "out; (for value in values { if *value > 1 { continue; } } )",
        "out; (while ready { break; })",
        "out; ('outer: while ready { for _ in values { continue 'outer; } })",
        "out; (async { return 5; }.await)",
        "out; ({ fn helper() -> i32 { return 6; } helper() })",
        "out; (value?)",
        "out; (format!(\"{value}\"))",
        "out; (matches!(value, Some(1)))",
    ] {
        if let Err(error) = syn::parse_str::<Template>(source) {
            panic!("rejected confined control flow: {source}: {error}");
        }
    }
}

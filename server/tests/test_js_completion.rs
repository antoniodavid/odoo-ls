use odoo_ls_server::core::js_index::JsModuleIndex;
use odoo_ls_server::core::js_parser::{JsComponent, JsModuleInfo};
use odoo_ls_server::features::js_completion::JsCompletionFeature;

#[test]
fn test_js_completion_contexts() {
    let mut index = JsModuleIndex::new();
    index
        .services
        .insert("foo".to_string(), (std::path::PathBuf::new(), (0, 0)));
    index
        .templates
        .insert("web.Foo".to_string(), (std::path::PathBuf::new(), (0, 0)));

    let content = r#"
import { Component } from "@odoo/owl";
export class Test extends Component {
    static template = "web.";
    setup() {
        useService("fo");
    }
}
    "#;

    // In template string
    let result = JsCompletionFeature::autocomplete(content, 3, 26, &index, None).unwrap();
    let items = match result {
        lsp_types::CompletionResponse::List(l) => l.items,
        _ => vec![],
    };
    assert!(items.iter().any(|i| i.label == "web.Foo"));

    // In useService
    let result = JsCompletionFeature::autocomplete(content, 5, 22, &index, None).unwrap();
    let items = match result {
        lsp_types::CompletionResponse::List(l) => l.items,
        _ => vec![],
    };
    assert!(items.iter().any(|i| i.label == "foo"));

    // In setup body
    let result = JsCompletionFeature::autocomplete(content, 4, 13, &index, None).unwrap();
    let items = match result {
        lsp_types::CompletionResponse::List(l) => l.items,
        _ => vec![],
    };
    assert!(items.iter().any(|i| i.label == "useState"));

    // In class body
    let result = JsCompletionFeature::autocomplete(content, 3, 4, &index, None).unwrap();
    let items = match result {
        lsp_types::CompletionResponse::List(l) => l.items,
        _ => vec![],
    };
    assert!(items.iter().any(|i| i.label == "static template"));
}

use odoo_ls_server::core::js_index::JsModuleIndex;
use odoo_ls_server::features::js_completion::JsCompletionFeature;

#[test]
fn test_import_completion() {
    let index = JsModuleIndex::new();
    let content = "import { use } from \"@odoo/owl\"";
    
    // In import specifier (line 0, character 12)
    let result = JsCompletionFeature::autocomplete(content, 0, 12, &index, None).unwrap();
    let items = match result { lsp_types::CompletionResponse::List(l) => l.items, _ => vec![] };
    assert!(items.iter().any(|i| i.label == "useState"));
}

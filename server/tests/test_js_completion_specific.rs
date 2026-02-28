use odoo_ls_server::core::js_index::JsModuleIndex;
use odoo_ls_server::features::js_completion::JsCompletionFeature;

#[test]
fn test_specific_setup_completion() {
    let index = JsModuleIndex::new();
    let content = r#"
export class SomeComponent extends Component {
    setup() {
        this.nextSourceId = 0;
        this.nextOptionId = 0;
        this.sources = [];
        this.inEdition = false;
        this.timeout = 250;
        
        this.state = useSta
    }
}
    "#;
    
    // Line 9 (0-indexed is 9), character 27
    let result = JsCompletionFeature::autocomplete(content, 9, 27, &index, None).unwrap();
    let items = match result { lsp_types::CompletionResponse::List(l) => l.items, _ => vec![] };
    let has_use_state = items.iter().any(|i| i.label == "useState");
    assert!(has_use_state, "Should have useState completion");
}

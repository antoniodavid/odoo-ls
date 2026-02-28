use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};
use std::path::{Path, PathBuf};
use crate::core::js_index::JsModuleIndex;
use crate::core::js_parser::JsModuleInfo;
use tree_sitter::{Parser, Point, Node};
use std::sync::Mutex;

static JS_PARSER: std::sync::LazyLock<Mutex<Parser>> = std::sync::LazyLock::new(|| {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_javascript::LANGUAGE.into())
        .expect("Error loading Javascript grammar");
    Mutex::new(parser)
});

pub struct JsHoverFeature;

impl JsHoverFeature {
    pub fn get_hover(
        content: &str,
        line: u32,
        character: u32,
        current_file: &PathBuf,
        index: &JsModuleIndex,
        _module_info: Option<&JsModuleInfo>,
    ) -> Option<Hover> {
        let mut parser = JS_PARSER.lock().unwrap();
        let tree = parser.parse(content, None)?;
        let root = tree.root_node();
        
        let point = Point::new(line as usize, character as usize);
        let node = root.descendant_for_point_range(point, point)?;

        let mut n = node;
        let text = n.utf8_text(content.as_bytes()).unwrap_or("").to_string();

        while let Some(parent) = n.parent() {
            let kind = parent.kind();

            // 1. Hover on import source
            if kind == "string" || kind == "string_fragment" {
                let clean_text = text.trim_matches('"').trim_matches('\'').trim_matches('`').to_string();
                let check_node = if kind == "string_fragment" { parent } else { n };
                if let Some(grandparent) = check_node.parent() {
                    if grandparent.kind() == "import_statement" {
                        if let Some(resolved_path) = index.resolve_import(&clean_text, current_file) {
                            return Some(Hover {
                                contents: HoverContents::Markup(MarkupContent {
                                    kind: MarkupKind::Markdown,
                                    value: format!("**Resolves to:**\n`{}`", resolved_path.display()),
                                }),
                                range: None,
                            });
                        }
                    }
                    
                    if grandparent.kind() == "arguments" {
                        if let Some(call) = grandparent.parent() {
                            if call.kind() == "call_expression" {
                                if let Some(func) = call.child_by_field_name("function") {
                                    if func.utf8_text(content.as_bytes()).unwrap_or("") == "useService" {
                                        if let Some((path, _)) = index.services.get(&clean_text) {
                                            return Some(Hover {
                                                contents: HoverContents::Markup(MarkupContent {
                                                    kind: MarkupKind::Markdown,
                                                    value: format!("**Odoo Service**: `{}`\n\nDefined in: `{}`", clean_text, path.file_name().unwrap_or_default().to_string_lossy()),
                                                }),
                                                range: None,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // 2. Hover on identifier
            if kind == "identifier" {
                // Is it an OWL component?
                if let Some((path, comp)) = index.components.get(&text) {
                    let mut props_str = String::new();
                    if !comp.props_keys.is_empty() {
                        props_str = format!("\n**Props**: {}", comp.props_keys.join(", "));
                    }
                    
                    let mut tpl_str = String::new();
                    if let Some(tpl) = &comp.template {
                        tpl_str = format!("\n**Template**: `{}`", tpl);
                    }
                    
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "```javascript\nclass {} extends Component\n```\nDefined in: `{}`\n{}{}", 
                                text, 
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                tpl_str,
                                props_str
                            ),
                        }),
                        range: None,
                    });
                }
                
                // Is it an OWL hook?
                if text == "useState" {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: "```javascript\nuseState(initialState)\n```\nCreates a reactive state object. Any changes to this object will trigger a re-render of the component.".to_string(),
                        }),
                        range: None,
                    });
                }
                if text == "onMounted" {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: "```javascript\nonMounted(callback)\n```\nLifecycle hook called right after the component is mounted to the DOM.".to_string(),
                        }),
                        range: None,
                    });
                }
                if text == "useService" {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: "```javascript\nuseService(serviceName)\n```\nGets a reference to an Odoo service. It must be called synchronously in the `setup()` method.".to_string(),
                        }),
                        range: None,
                    });
                }
            }

            n = parent;
        }

        None
    }
}

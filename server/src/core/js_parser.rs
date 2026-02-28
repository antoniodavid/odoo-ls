use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator, TreeCursor};

static JS_PARSER: std::sync::LazyLock<Mutex<Parser>> = std::sync::LazyLock::new(|| {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .expect("Error loading Javascript grammar");
    Mutex::new(parser)
});

// Query to match import statements
static IMPORT_QUERY: std::sync::LazyLock<Query> = std::sync::LazyLock::new(|| {
    Query::new(
        &tree_sitter_javascript::LANGUAGE.into(),
        r#"
        (import_statement
            (import_clause 
                (named_imports 
                    (import_specifier 
                        name: (identifier) @import.name 
                        alias: (identifier)? @import.alias
                    )
                )
            )
            source: (string) @import.source
        ) @import_stmt
        "#,
    )
    .unwrap()
});

// Query to match components (class extending Component)
static COMPONENT_QUERY: std::sync::LazyLock<Query> = std::sync::LazyLock::new(|| {
    Query::new(
        &tree_sitter_javascript::LANGUAGE.into(),
        r#"
        (export_statement
            declaration: (class_declaration
                name: (identifier) @class.name
                (class_heritage (identifier) @class.extends)
                body: (class_body) @class.body
            )
        ) @export_stmt
        
        (class_declaration
            name: (identifier) @class.name
            (class_heritage (identifier) @class.extends)
            body: (class_body) @class.body
        ) @class_stmt
        "#,
    )
    .unwrap()
});

#[derive(Debug, Clone, Default)]
pub struct JsModuleInfo {
    pub imports: Vec<JsImport>,
    pub components: Vec<JsComponent>,
    pub registry_calls: Vec<JsRegistryCall>,
    pub service_usages: Vec<JsServiceUsage>,
}

#[derive(Debug, Clone)]
pub struct JsImport {
    pub specifiers: Vec<JsImportSpecifier>,
    pub source: String,
    pub range: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct JsImportSpecifier {
    pub name: String,
    pub alias: Option<String>,
    pub range: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct JsComponent {
    pub name: String,
    pub extends: String,
    pub template: Option<String>,
    pub props_keys: Vec<String>,
    pub sub_components: Vec<String>,
    pub setup_range: Option<(usize, usize)>,
    pub range: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct JsRegistryCall {
    pub category: String,
    pub key: String,
    pub range: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct JsServiceUsage {
    pub service_name: String,
    pub range: (usize, usize),
}

pub struct JsParser;

impl JsParser {
    pub fn parse(content: &str) -> Option<JsModuleInfo> {
        let mut parser = JS_PARSER.lock().unwrap();
        let tree = parser.parse(content, None)?;

        let mut info = JsModuleInfo::default();
        let root = tree.root_node();

        Self::extract_imports(content, root, &mut info);
        Self::extract_components(content, root, &mut info);
        Self::extract_registries_and_services(content, root, &mut info);

        Some(info)
    }

    fn extract_imports(content: &str, root: Node, info: &mut JsModuleInfo) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&IMPORT_QUERY, root, content.as_bytes());

        // Group by statement
        let mut statements: HashMap<usize, JsImport> = HashMap::new();

        while let Some(m) = matches.next() {
            let mut stmt_node: Option<Node> = None;
            let mut source = None;
            let mut specifier = JsImportSpecifier {
                name: String::new(),
                alias: None,
                range: (0, 0),
            };

            for capture in m.captures {
                let capture_name: &str = &IMPORT_QUERY.capture_names()[capture.index as usize];
                let text = capture
                    .node
                    .utf8_text(content.as_bytes())
                    .unwrap_or("")
                    .to_string();

                match capture_name {
                    "import_stmt" => stmt_node = Some(capture.node),
                    "import.source" => {
                        source = Some(text.trim_matches('"').trim_matches('\'').to_string())
                    }
                    "import.name" => {
                        specifier.name = text;
                        specifier.range = (capture.node.start_byte(), capture.node.end_byte());
                    }
                    "import.alias" => specifier.alias = Some(text),
                    _ => {}
                }
            }

            if let (Some(stmt), Some(src)) = (stmt_node, source) {
                let stmt_id: usize = stmt.id();
                let import = statements.entry(stmt_id).or_insert(JsImport {
                    specifiers: Vec::new(),
                    source: src,
                    range: (stmt.start_byte(), stmt.end_byte()),
                });
                import.specifiers.push(specifier);
            }
        }

        info.imports = statements.into_values().collect();
    }

    fn extract_components(content: &str, root: Node, info: &mut JsModuleInfo) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&COMPONENT_QUERY, root, content.as_bytes());

        let mut processed_classes = HashSet::new();

        while let Some(m) = matches.next() {
            let mut name = String::new();
            let mut extends = String::new();
            let mut body_node: Option<Node> = None;
            let mut class_node: Option<Node> = None;

            for capture in m.captures {
                let capture_name: &str = &COMPONENT_QUERY.capture_names()[capture.index as usize];
                let text = capture
                    .node
                    .utf8_text(content.as_bytes())
                    .unwrap_or("")
                    .to_string();

                match capture_name {
                    "class.name" => {
                        name = text;
                        class_node = Some(capture.node.parent().unwrap()); // This will be the class_declaration
                    }
                    "class.extends" => extends = text,
                    "class.body" => body_node = Some(capture.node),
                    _ => {}
                }
            }

            // Only process components (extends Component)
            if extends != "Component" && extends != "Component"
                || class_node.is_none()
                || body_node.is_none()
            {
                continue;
            }

            let class = class_node.unwrap();
            let body = body_node.unwrap();

            if !processed_classes.insert(class.id()) {
                continue; // Already processed
            }

            let mut component = JsComponent {
                name,
                extends,
                template: None,
                props_keys: Vec::new(),
                sub_components: Vec::new(),
                setup_range: None,
                range: (class.start_byte(), class.end_byte()),
            };

            // Extract static properties and methods
            let mut walker = body.walk();
            for child in body.children(&mut walker) {
                if child.kind() == "method_definition" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if name_node.utf8_text(content.as_bytes()).unwrap_or("") == "setup" {
                            component.setup_range = Some((child.start_byte(), child.end_byte()));
                        }
                    }
                } else if child.kind() == "field_definition"
                    || child.kind() == "public_field_definition"
                {
                    // Check static template, static props, static components
                    let is_static = child
                        .children(&mut child.walk())
                        .any(|n| n.kind() == "static");
                    if !is_static {
                        continue;
                    }

                    if let Some(name_node) = child
                        .child_by_field_name("property")
                        .or_else(|| child.child_by_field_name("name"))
                    {
                        let field_name = name_node.utf8_text(content.as_bytes()).unwrap_or("");

                        if field_name == "template" {
                            if let Some(val_node) = child.child_by_field_name("value") {
                                // Extract string or template string
                                let val_text = val_node.utf8_text(content.as_bytes()).unwrap_or("");
                                // Clean quotes/backticks
                                let clean_val = val_text
                                    .trim_start_matches("xml")
                                    .trim_matches('`')
                                    .trim_matches('"')
                                    .trim_matches('\'')
                                    .to_string();
                                component.template = Some(clean_val);
                            }
                        } else if field_name == "props" {
                            if let Some(val_node) = child.child_by_field_name("value") {
                                if val_node.kind() == "object" {
                                    for pair in val_node.children(&mut val_node.walk()) {
                                        if pair.kind() == "pair" {
                                            if let Some(key) = pair.child_by_field_name("key") {
                                                component.props_keys.push(
                                                    key.utf8_text(content.as_bytes())
                                                        .unwrap_or("")
                                                        .to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        } else if field_name == "components" {
                            if let Some(val_node) = child.child_by_field_name("value") {
                                if val_node.kind() == "object" {
                                    for prop in val_node.children(&mut val_node.walk()) {
                                        if prop.kind() == "shorthand_property_identifier" {
                                            component.sub_components.push(
                                                prop.utf8_text(content.as_bytes())
                                                    .unwrap_or("")
                                                    .to_string(),
                                            );
                                        } else if prop.kind() == "pair" {
                                            if let Some(key) = prop.child_by_field_name("key") {
                                                component.sub_components.push(
                                                    key.utf8_text(content.as_bytes())
                                                        .unwrap_or("")
                                                        .to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            info.components.push(component);
        }
    }

    fn extract_registries_and_services(content: &str, root: Node, info: &mut JsModuleInfo) {
        let mut cursor = root.walk();
        Self::walk_for_registries_and_services(content, root, info, &mut cursor);
    }

    fn walk_for_registries_and_services<'a>(
        content: &str,
        node: Node<'a>,
        info: &mut JsModuleInfo,
        cursor: &mut TreeCursor<'a>,
    ) {
        // Look for: registry.category("foo").add("bar", ...)
        if node.kind() == "call_expression" {
            let code = node.utf8_text(content.as_bytes()).unwrap_or("");
            if code.starts_with("registry.category(") && code.contains(".add(") {
                // Manually extract using basic string manipulation since it's a known reliable pattern in Odoo
                if let Some(cat_start) = code.find("(\"").or(code.find("('")) {
                    if let Some(cat_end) = code[cat_start + 2..]
                        .find("\"")
                        .or(code[cat_start + 2..].find("'"))
                    {
                        let category = &code[cat_start + 2..cat_start + 2 + cat_end];

                        if let Some(add_idx) = code.find(".add(") {
                            let add_args = &code[add_idx + 5..];
                            if let Some(key_start) = add_args.find("\"").or(add_args.find("'")) {
                                if let Some(key_end) = add_args[key_start + 1..]
                                    .find("\"")
                                    .or(add_args[key_start + 1..].find("'"))
                                {
                                    let key = &add_args[key_start + 1..key_start + 1 + key_end];

                                    info.registry_calls.push(JsRegistryCall {
                                        category: category.to_string(),
                                        key: key.to_string(),
                                        range: (node.start_byte(), node.end_byte()),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            // Look for: useService("foo")
            else if code.starts_with("useService(") {
                if let Some(args_node) = node.child_by_field_name("arguments") {
                    if let Some(arg) = args_node.child(1) {
                        // First arg after '('
                        if arg.kind() == "string" {
                            let text = arg.utf8_text(content.as_bytes()).unwrap_or("");
                            let service_name =
                                text.trim_matches('"').trim_matches('\'').to_string();
                            info.service_usages.push(JsServiceUsage {
                                service_name,
                                range: (node.start_byte(), node.end_byte()),
                            });
                        }
                    }
                }
            }
        }

        for child in node.children(cursor) {
            Self::walk_for_registries_and_services(content, child, info, &mut child.walk());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_imports() {
        let code = r#"
            import { Component, useState } from "@odoo/owl";
            import { useService as us } from "@web/core/utils/hooks";
        "#;

        let info = JsParser::parse(code).unwrap();
        assert_eq!(info.imports.len(), 2);

        let owl_import = info
            .imports
            .iter()
            .find(|i| i.source == "@odoo/owl")
            .unwrap();
        assert_eq!(owl_import.specifiers.len(), 2);
        assert_eq!(owl_import.specifiers[0].name, "Component");
        assert_eq!(owl_import.specifiers[1].name, "useState");

        let web_import = info
            .imports
            .iter()
            .find(|i| i.source == "@web/core/utils/hooks")
            .unwrap();
        assert_eq!(web_import.specifiers.len(), 1);
        assert_eq!(web_import.specifiers[0].name, "useService");
        assert_eq!(web_import.specifiers[0].alias.as_deref(), Some("us"));
    }

    #[test]
    fn test_parse_components() {
        let code = r#"
            export class Colorpicker extends Component {
                static template = "web.Colorpicker";
                static props = {
                    document: { type: true, optional: true },
                    defaultColor: { type: String, optional: true },
                };
                static components = { Widget, ChildComponent };

                setup() {
                    this.state = useState({});
                }
            }
        "#;

        let mut parser = JS_PARSER.lock().unwrap();
        let tree = parser.parse(code, None).unwrap();
        println!("Tree: {}", tree.root_node().to_sexp());
        drop(parser);

        let info = JsParser::parse(code).unwrap();
        assert_eq!(info.components.len(), 1);

        let comp = &info.components[0];
        assert_eq!(comp.name, "Colorpicker");
        assert_eq!(comp.extends, "Component");
        assert_eq!(comp.template.as_deref(), Some("web.Colorpicker"));
        assert_eq!(comp.props_keys, vec!["document", "defaultColor"]);
        assert_eq!(comp.sub_components, vec!["Widget", "ChildComponent"]);
        assert!(comp.setup_range.is_some());
    }

    #[test]
    fn test_parse_registries_and_services() {
        let code = r#"
            registry.category("fields").add("many2one", many2OneField);
            
            export class Test extends Component {
                setup() {
                    this.orm = useService("orm");
                    useService('action');
                }
            }
        "#;

        let info = JsParser::parse(code).unwrap();

        assert_eq!(info.registry_calls.len(), 1);
        assert_eq!(info.registry_calls[0].category, "fields");
        assert_eq!(info.registry_calls[0].key, "many2one");

        assert_eq!(info.service_usages.len(), 2);
        assert_eq!(info.service_usages[0].service_name, "orm");
        assert_eq!(info.service_usages[1].service_name, "action");
    }
}

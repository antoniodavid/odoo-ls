use crate::core::file_mgr::FileMgr;
use crate::core::js_index::JsModuleIndex;
use crate::core::js_parser::JsModuleInfo;
use lsp_types::{GotoDefinitionResponse, Location, Position, Range};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tree_sitter::{Node, Parser, Point};

static JS_PARSER: std::sync::LazyLock<Mutex<Parser>> = std::sync::LazyLock::new(|| {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .expect("Error loading Javascript grammar");
    Mutex::new(parser)
});

pub struct JsDefinitionFeature;

impl JsDefinitionFeature {
    pub fn get_location(
        content: &str,
        line: u32,
        character: u32,
        current_file: &PathBuf,
        index: &JsModuleIndex,
        _module_info: Option<&JsModuleInfo>,
    ) -> Option<GotoDefinitionResponse> {
        let mut parser = JS_PARSER.lock().unwrap();
        let tree = parser.parse(content, None)?;
        let root = tree.root_node();

        let point = Point::new(line as usize, character as usize);
        let node = root.descendant_for_point_range(point, point)?;

        let mut n = node;
        let text = n.utf8_text(content.as_bytes()).unwrap_or("").to_string();

        while let Some(parent) = n.parent() {
            let kind = parent.kind();

            // 1. Go to definition for string literals
            if kind == "string" || kind == "string_fragment" {
                let clean_text = text
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim_matches('`')
                    .to_string();

                // If it's a string_fragment, parent is string, grandparent is argument/etc
                let check_node = if kind == "string_fragment" { parent } else { n };

                if let Some(grandparent) = check_node.parent() {
                    // 1a. Import paths: import ... from "@web/core/utils/hooks"
                    if grandparent.kind() == "import_statement" {
                        if let Some(resolved_path) = index.resolve_import(&clean_text, current_file)
                        {
                            return Self::create_location(&resolved_path, 0, 0);
                        }
                    }

                    // 1b. useService("orm") -> Find service definition
                    if grandparent.kind() == "arguments" {
                        if let Some(call) = grandparent.parent() {
                            if call.kind() == "call_expression" {
                                if let Some(func) = call.child_by_field_name("function") {
                                    if func.utf8_text(content.as_bytes()).unwrap_or("")
                                        == "useService"
                                    {
                                        if let Some((path, _range)) =
                                            index.services.get(&clean_text)
                                        {
                                            return Self::create_location(path, 0, 0);
                                            // TODO: use real line from _range
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 1c. static template = "web.Colorpicker" -> Find XML template
                    if grandparent.kind() == "field_definition" {
                        if let Some(prop) = grandparent.child_by_field_name("property") {
                            if prop.utf8_text(content.as_bytes()).unwrap_or("") == "template" {
                                // Handled via global XML cache usually, but we could find it in JsModuleIndex.templates
                                if let Some((path, _)) = index.templates.get(&clean_text) {
                                    return Self::create_location(path, 0, 0);
                                }
                            }
                        }
                    }
                }
            }

            // 2. Component references in `static components = { MyComponent }`
            if kind == "shorthand_property_identifier" || kind == "identifier" {
                if let Some(grandparent) = parent.parent() {
                    if grandparent.kind() == "object" {
                        if let Some(field_def) = grandparent.parent() {
                            if field_def.kind() == "field_definition" {
                                if let Some(prop) = field_def.child_by_field_name("property") {
                                    if prop.utf8_text(content.as_bytes()).unwrap_or("")
                                        == "components"
                                    {
                                        if let Some((path, _comp)) = index.components.get(&text) {
                                            return Self::create_location(path, 0, 0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 3. Import specifiers: import { Component } from ...
            if kind == "import_specifier" {
                if let Some(name) = parent.child_by_field_name("name") {
                    let _specifier_name = name.utf8_text(content.as_bytes()).unwrap_or("");
                    if let Some(import_stmt) = Self::find_parent_of_kind(parent, "import_statement")
                    {
                        if let Some(source_node) = import_stmt.child_by_field_name("source") {
                            let source = source_node
                                .utf8_text(content.as_bytes())
                                .unwrap_or("")
                                .trim_matches('"')
                                .trim_matches('\'');
                            if let Some(resolved_path) = index.resolve_import(source, current_file)
                            {
                                return Self::create_location(&resolved_path, 0, 0);
                            }
                        }
                    }
                }
            }

            // 4. Any identifier that matches an import
            if kind == "identifier" {
                if let Some(info) = _module_info {
                    for import in &info.imports {
                        for specifier in &import.specifiers {
                            let match_name = specifier.alias.as_ref().unwrap_or(&specifier.name);
                            if match_name == &text {
                                if let Some(resolved_path) =
                                    index.resolve_import(&import.source, current_file)
                                {
                                    return Self::create_location(&resolved_path, 0, 0);
                                }
                            }
                        }
                    }
                }
            }

            n = parent;
        }

        None
    }

    fn find_parent_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
        let mut curr = node;
        while let Some(parent) = curr.parent() {
            if parent.kind() == kind {
                return Some(parent);
            }
            curr = parent;
        }
        None
    }

    fn create_location(
        path: &PathBuf,
        line: u32,
        character: u32,
    ) -> Option<GotoDefinitionResponse> {
        let uri = FileMgr::pathname2uri(&path.to_string_lossy().into_owned());
        Some(GotoDefinitionResponse::Scalar(Location {
            uri,
            range: Range {
                start: Position { line, character },
                end: Position { line, character },
            },
        }))
    }
}

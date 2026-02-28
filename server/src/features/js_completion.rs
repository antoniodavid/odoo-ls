use crate::core::js_index::JsModuleIndex;
use crate::core::js_parser::JsModuleInfo;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionResponse, Documentation,
    InsertTextFormat, MarkupContent, MarkupKind,
};
use std::sync::Mutex;
use tree_sitter::{Parser, Point};

static JS_PARSER: std::sync::LazyLock<Mutex<Parser>> = std::sync::LazyLock::new(|| {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .expect("Error loading Javascript grammar");
    Mutex::new(parser)
});

/// Provides OWL (Odoo Web Library) completions for JavaScript files using the global JsModuleIndex.
pub struct JsCompletionFeature;

#[derive(Debug, Clone, PartialEq)]
enum JsContext {
    /// Cursor is inside a `setup()` method body
    SetupMethod,
    /// Cursor is inside a class body that extends Component
    ComponentClassBody,
    /// Cursor is inside an import statement (from clause)
    ImportStatement,
    /// Cursor is inside named imports list
    ImportSpecifier,
    /// Cursor is inside useService("...") string
    ServiceName,
    /// Cursor is inside static template = "..." string
    TemplateName,
    /// Cursor is inside registry.category("...") string
    RegistryCategory,
    /// Fallback
    TopLevel,
}

impl JsCompletionFeature {
    pub fn autocomplete(
        content: &str,
        line: u32,
        character: u32,
        index: &JsModuleIndex,
        _module_info: Option<&JsModuleInfo>,
    ) -> Option<CompletionResponse> {
        let (context, _node_text) = Self::detect_context(content, line, character);
        let items = match context {
            JsContext::SetupMethod => Self::setup_completions(),
            JsContext::ComponentClassBody => Self::class_body_completions(),
            JsContext::ImportStatement => Self::import_path_completions(index),
            JsContext::ImportSpecifier => Self::import_specifier_completions(),
            JsContext::ServiceName => Self::service_name_completions(index),
            JsContext::TemplateName => Self::template_name_completions(index),
            JsContext::RegistryCategory => Self::registry_category_completions(index),
            _ => {
                // Return everything just in case detection failed, letting nvim-cmp filter
                let mut fallback = Self::import_completions();
                fallback.extend(Self::setup_completions());
                fallback
            }
        };

        if items.is_empty() {
            return None;
        }

        Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items,
        }))
    }

    /// Uses tree-sitter to find the innermost node at the cursor and determine the context
    fn detect_context(content: &str, line: u32, character: u32) -> (JsContext, String) {
        let mut parser = JS_PARSER.lock().unwrap();
        let tree = parser.parse(content, None);
        if tree.is_none() {
            return (JsContext::TopLevel, String::new());
        }
        let tree = tree.unwrap();
        let root = tree.root_node();

        let point = Point::new(line as usize, character as usize);
        let node = root.descendant_for_point_range(point, point);

        if let Some(mut n) = node {
            println!("Cursor at node: {:?} (kind: {})", n, n.kind());
            let text = n.utf8_text(content.as_bytes()).unwrap_or("").to_string();

            // Walk up the syntax tree to find context
            while let Some(parent) = n.parent() {
                let kind = parent.kind();

                // Inside string literal
                if kind == "string" {
                    if let Some(grandparent) = parent.parent() {
                        // useService("...")
                        if grandparent.kind() == "arguments" {
                            if let Some(call) = grandparent.parent() {
                                if call.kind() == "call_expression" {
                                    if let Some(func) = call.child_by_field_name("function") {
                                        if func.utf8_text(content.as_bytes()).unwrap_or("")
                                            == "useService"
                                        {
                                            return (JsContext::ServiceName, text);
                                        }
                                    }
                                }
                            }
                        }
                        // registry.category("...")
                        if grandparent.kind() == "arguments" {
                            if let Some(call) = grandparent.parent() {
                                if call.kind() == "call_expression" {
                                    if let Some(func) = call.child_by_field_name("function") {
                                        let func_text =
                                            func.utf8_text(content.as_bytes()).unwrap_or("");
                                        if func_text == "registry.category" {
                                            return (JsContext::RegistryCategory, text);
                                        }
                                    }
                                }
                            }
                        }
                        // import "..."
                        if grandparent.kind() == "import_statement" {
                            return (JsContext::ImportStatement, text);
                        }
                        // static template = "..."
                        if grandparent.kind() == "field_definition" {
                            if let Some(prop) = grandparent.child_by_field_name("property") {
                                if prop.utf8_text(content.as_bytes()).unwrap_or("") == "template" {
                                    return (JsContext::TemplateName, text);
                                }
                            }
                        }
                    }
                }

                if kind == "named_imports" {
                    return (JsContext::ImportSpecifier, text);
                }

                if kind == "method_definition" {
                    if let Some(name) = parent.child_by_field_name("name") {
                        if name.utf8_text(content.as_bytes()).unwrap_or("") == "setup" {
                            return (JsContext::SetupMethod, text);
                        }
                    }
                }

                if kind == "class_body" {
                    if let Some(class_decl) = parent.parent() {
                        // Check if the class has a heritage child (extends)
                        let has_heritage = class_decl
                            .children(&mut class_decl.walk())
                            .any(|c| c.kind() == "class_heritage");
                        if has_heritage {
                            return (JsContext::ComponentClassBody, text);
                        }
                    }
                }

                n = parent;
            }
        }

        (JsContext::TopLevel, String::new())
    }

    fn import_specifier_completions() -> Vec<CompletionItem> {
        let mut items = Vec::new();
        // OWL hooks and utils
        let exports = [
            "Component",
            "useState",
            "useRef",
            "useEnv",
            "useSubEnv",
            "useEffect",
            "onMounted",
            "onWillStart",
            "onWillUnmount",
            "onPatched",
            "onWillPatch",
            "useService",
            "useBus",
            "useAutofocus",
            "useChildRef",
            "useOwnedDialogs",
        ];

        for name in exports {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("Odoo / OWL Export".to_string()),
                ..Default::default()
            });
        }
        items
    }

    fn service_name_completions(index: &JsModuleIndex) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        // Add hardcoded common ones as fallback
        let common = [
            "orm",
            "action",
            "dialog",
            "notification",
            "ui",
            "view",
            "company",
            "http",
            "rpc",
        ];
        for name in common {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::INTERFACE),
                detail: Some("Odoo Core Service".to_string()),
                ..Default::default()
            });
        }
        // Add dynamic ones from index
        for (name, _) in &index.services {
            if !common.contains(&name.as_str()) {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::INTERFACE),
                    detail: Some("Odoo Addon Service".to_string()),
                    ..Default::default()
                });
            }
        }
        items
    }

    fn template_name_completions(index: &JsModuleIndex) -> Vec<CompletionItem> {
        index
            .templates
            .keys()
            .map(|name| CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("QWeb Template".to_string()),
                ..Default::default()
            })
            .collect()
    }

    fn registry_category_completions(index: &JsModuleIndex) -> Vec<CompletionItem> {
        let mut categories: Vec<String> = index.registry_entries.keys().cloned().collect();
        // Add standard ones if index is missing them
        for c in [
            "services",
            "fields",
            "views",
            "actions",
            "systray",
            "main_components",
            "view_widgets",
        ] {
            if !categories.contains(&c.to_string()) {
                categories.push(c.to_string());
            }
        }
        categories
            .into_iter()
            .map(|name| CompletionItem {
                label: name,
                kind: Some(CompletionItemKind::FOLDER),
                detail: Some("Registry Category".to_string()),
                ..Default::default()
            })
            .collect()
    }

    fn import_path_completions(index: &JsModuleIndex) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        items.push(CompletionItem {
            label: "@odoo/owl".to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("OWL Framework".to_string()),
            ..Default::default()
        });
        for path in index.module_paths.keys() {
            items.push(CompletionItem {
                label: path.to_string(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some("Odoo Module".to_string()),
                ..Default::default()
            });
        }
        items
    }

    /// Completions available inside a `setup()` method
    fn setup_completions() -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // OWL lifecycle hooks
        let lifecycle_hooks = [
            (
                "onWillStart",
                "async () => {\n\t$0\n}",
                "Called before the component is mounted. Can be async.",
                "@odoo/owl",
            ),
            (
                "onMounted",
                "() => {\n\t$0\n}",
                "Called after the component is mounted to the DOM.",
                "@odoo/owl",
            ),
            (
                "onWillUpdateProps",
                "async (nextProps) => {\n\t$0\n}",
                "Called before the component receives new props. Can be async.",
                "@odoo/owl",
            ),
            (
                "onWillPatch",
                "() => {\n\t$0\n}",
                "Called before the DOM is patched.",
                "@odoo/owl",
            ),
            (
                "onPatched",
                "() => {\n\t$0\n}",
                "Called after the DOM is patched.",
                "@odoo/owl",
            ),
            (
                "onWillUnmount",
                "() => {\n\t$0\n}",
                "Called before the component is unmounted.",
                "@odoo/owl",
            ),
            (
                "onWillDestroy",
                "() => {\n\t$0\n}",
                "Called before the component is destroyed.",
                "@odoo/owl",
            ),
            (
                "onWillRender",
                "() => {\n\t$0\n}",
                "Called before each render.",
                "@odoo/owl",
            ),
            (
                "onRendered",
                "() => {\n\t$0\n}",
                "Called after each render.",
                "@odoo/owl",
            ),
            (
                "onError",
                "(error) => {\n\t$0\n}",
                "Called when an error occurs in a child component.",
                "@odoo/owl",
            ),
        ];

        for (i, (name, snippet, doc, source)) in lifecycle_hooks.iter().enumerate() {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(format!("OWL lifecycle hook ({source})")),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```javascript\n{name}(callback)\n```\n\n{doc}"),
                })),
                insert_text: Some(format!("{name}({snippet})")),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some(format!("0a{i:02}")),
                ..Default::default()
            });
        }

        // OWL composition hooks
        let composition_hooks = [
            (
                "useRef",
                "\"${1:refName}\"",
                "Returns a reference object for a DOM element using t-ref.",
                "@odoo/owl",
            ),
            (
                "useState",
                "{$0}",
                "Creates a reactive state object.",
                "@odoo/owl",
            ),
            (
                "useEnv",
                "",
                "Returns the component's environment.",
                "@odoo/owl",
            ),
            (
                "useSubEnv",
                "{$0}",
                "Extends the environment for child components.",
                "@odoo/owl",
            ),
            (
                "useChildSubEnv",
                "{$0}",
                "Extends the environment for child components (not the component itself).",
                "@odoo/owl",
            ),
            (
                "useEffect",
                "() => {\n\t$0\n}, () => []",
                "Registers a side effect that runs after rendering.",
                "@odoo/owl",
            ),
            (
                "useExternalListener",
                "${1:target}, \"${2:event}\", ${3:handler}",
                "Adds an event listener on an external target, auto-cleaned on unmount.",
                "@odoo/owl",
            ),
            (
                "useComponent",
                "",
                "Returns the current component instance.",
                "@odoo/owl",
            ),
        ];

        for (i, (name, snippet, doc, source)) in composition_hooks.iter().enumerate() {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(format!("OWL hook ({source})")),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```javascript\n{name}({snippet})\n```\n\n{doc}")
                        .replace("${1:", "")
                        .replace("${2:", "")
                        .replace("${3:", "")
                        .replace("}", "")
                        .replace("$0", "..."),
                })),
                insert_text: Some(format!("{name}({snippet})")),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some(format!("0b{i:02}")),
                ..Default::default()
            });
        }

        // Odoo-specific hooks
        let odoo_hooks = [
            (
                "useService",
                "\"${1:serviceName}\"",
                "Access an Odoo service (e.g. 'orm', 'notification', 'action').",
                "@web/core/utils/hooks",
            ),
            (
                "useBus",
                "${1:bus}, \"${2:eventName}\", ${3:callback}",
                "Subscribe to a bus event, auto-cleaned on unmount.",
                "@web/core/utils/hooks",
            ),
            (
                "useAutofocus",
                "{ refName: \"${1:autofocus}\" }",
                "Auto-focuses an input element on mount.",
                "@web/core/utils/hooks",
            ),
            (
                "useHotkey",
                "\"${1:hotkey}\", ${2:callback}",
                "Registers a keyboard shortcut.",
                "@web/core/hotkeys/hotkey_hook",
            ),
            (
                "useCommand",
                "\"${1:name}\", ${2:action}",
                "Registers a command in the command palette.",
                "@web/core/commands/command_hook",
            ),
            (
                "useDebounced",
                "${1:callback}, ${2:delay}",
                "Creates a debounced version of a function.",
                "@web/core/utils/timing",
            ),
            (
                "useThrottleForAnimation",
                "${1:callback}",
                "Creates a throttled function synced to animation frames.",
                "@web/core/utils/timing",
            ),
            (
                "usePosition",
                "\"${1:refName}\", ${2:getTarget}",
                "Positions an element relative to a target.",
                "@web/core/position/position_hook",
            ),
            (
                "usePopover",
                "${1:component}",
                "Creates a popover controller.",
                "@web/core/popover/popover_hook",
            ),
            (
                "useDropdownState",
                "",
                "Creates dropdown state management.",
                "@web/core/dropdown/dropdown_hooks",
            ),
            (
                "useSortable",
                "{$0}",
                "Makes a list sortable via drag and drop.",
                "@web/core/utils/sortable_owl",
            ),
            (
                "useOwnedDialogs",
                "",
                "Returns addDialog function for dialogs owned by this component.",
                "@web/core/utils/hooks",
            ),
            (
                "useChildRef",
                "",
                "Creates a ref that can be forwarded to a child component.",
                "@web/core/utils/hooks",
            ),
            (
                "useForwardRefToParent",
                "\"${1:refName}\"",
                "Forwards a ref to the parent component.",
                "@web/core/utils/hooks",
            ),
            (
                "useSpellCheck",
                "{ refName: \"${1:refName}\" }",
                "Enables/disables spell checking on an element.",
                "@web/core/utils/hooks",
            ),
            (
                "useActiveElement",
                "\"${1:refName}\"",
                "Registers the element as the active UI element.",
                "@web/core/ui/ui_service",
            ),
            (
                "useFileViewer",
                "",
                "Returns the file viewer controller.",
                "@web/core/file_viewer/file_viewer_hook",
            ),
            (
                "useNavigation",
                "${1:containerRef}",
                "Adds keyboard navigation to a container.",
                "@web/core/navigation/navigation",
            ),
            (
                "useDateTimePicker",
                "{$0}",
                "Creates a date/time picker controller.",
                "@web/core/datetime/datetime_hook",
            ),
        ];

        for (i, (name, snippet, doc, source)) in odoo_hooks.iter().enumerate() {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(format!("Odoo hook ({source})")),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```javascript\nimport {{ {name} }} from \"{source}\";\n```\n\n{doc}"
                    ),
                })),
                insert_text: Some(format!("{name}({snippet})")),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some(format!("0c{i:02}")),
                ..Default::default()
            });
        }

        items
    }

    /// Completions available inside a Component class body (outside setup)
    fn class_body_completions() -> Vec<CompletionItem> {
        vec![
            CompletionItem {
                label: "static template".to_string(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("OWL Component template".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "The QWeb template XML ID for this component.\n\n```javascript\nstatic template = \"module.TemplateName\";\n```".to_string(),
                })),
                insert_text: Some("static template = \"${1:module.TemplateName}\";".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some("0a00".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "static props".to_string(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("OWL Component props validation".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "Defines the expected props and their types.\n\n```javascript\nstatic props = {\n    name: { type: String },\n    count: { type: Number, optional: true },\n};\n```".to_string(),
                })),
                insert_text: Some("static props = {\n\t${0}\n};".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some("0a01".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "static defaultProps".to_string(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("OWL Component default props".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "Default values for optional props.\n\n```javascript\nstatic defaultProps = {\n    count: 0,\n};\n```".to_string(),
                })),
                insert_text: Some("static defaultProps = {\n\t${0}\n};".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some("0a02".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "static components".to_string(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("OWL sub-components".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "Declares sub-components used in the template.\n\n```javascript\nstatic components = { ChildComponent };\n```".to_string(),
                })),
                insert_text: Some("static components = { ${0} };".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some("0a03".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "setup()".to_string(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some("OWL Component setup method".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "The setup method is called during component construction.\nUse it to register hooks, create reactive state, and access services.\n\n```javascript\nsetup() {\n    this.state = useState({ ... });\n    useService(\"orm\");\n}\n```".to_string(),
                })),
                insert_text: Some("setup() {\n\t${0}\n}".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some("0a04".to_string()),
                ..Default::default()
            },
        ]
    }

    /// Import completions at the top level
    fn import_completions() -> Vec<CompletionItem> {
        vec![
            CompletionItem {
                label: "import { Component } from \"@odoo/owl\"".to_string(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some("OWL Component import".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "Import the OWL Component base class.\n\nAvailable exports from `@odoo/owl`:\n- `Component`, `App`, `EventBus`\n- `useState`, `useRef`, `useEnv`, `useSubEnv`, `useChildSubEnv`\n- `useEffect`, `useExternalListener`, `useComponent`\n- `onMounted`, `onWillStart`, `onWillUnmount`, `onPatched`, etc.\n- `xml`, `mount`, `status`, `reactive`, `markRaw`, `toRaw`".to_string(),
                })),
                insert_text: Some("import { ${1:Component} } from \"@odoo/owl\";".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some("0a00".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "import { useService } from \"@web/core/utils/hooks\"".to_string(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some("Odoo hooks import".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "Import Odoo utility hooks.\n\nAvailable exports from `@web/core/utils/hooks`:\n- `useService` — access Odoo services\n- `useBus` — subscribe to bus events\n- `useAutofocus` — auto-focus inputs\n- `useOwnedDialogs` — manage dialogs\n- `useChildRef` / `useForwardRefToParent` — ref forwarding\n- `useSpellCheck` — spell checking\n- `useRefListener` — ref event listeners".to_string(),
                })),
                insert_text: Some("import { ${1:useService} } from \"@web/core/utils/hooks\";".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some("0a01".to_string()),
                ..Default::default()
            },
        ]
    }
}

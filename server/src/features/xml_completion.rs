use std::{cell::RefCell, rc::Rc};
use lsp_types::{CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionList, CompletionResponse};

use crate::{
    core::{
        evaluation::Context,
        file_mgr::FileInfo,
        symbols::symbol::Symbol,
    },
    features::xml_ast_utils::{XmlAstResult, XmlAstUtils},
    threads::SessionInfo,
};

pub struct XmlCompletionFeature;

/// Helper function to extract field type from a field symbol.
/// Returns a string like "Integer", "Float", "(res.users) Many2one", etc.
fn get_field_type(session: &mut SessionInfo, symbol: &Rc<RefCell<Symbol>>) -> Option<String> {
    let sym_ref = symbol.borrow();

    if sym_ref.typ() != crate::constants::SymType::VARIABLE {
        return None;
    }

    let parent_context = sym_ref.parent().and_then(|parent| parent.upgrade());
    let evals_option = sym_ref.evaluations().cloned();

    drop(sym_ref);

    let Some(evals) = evals_option else {
        return None;
    };

    for eval in evals.iter() {
        let eval_symbol = eval.symbol.get_symbol(session, &mut None, &mut vec![], None);

        let mut context = None;
        if let Some(parent) = &parent_context {
            context = Some(Context::new());
            context.as_mut().unwrap().insert(
                crate::oyarn!("base_attr").to_string(),
                crate::core::evaluation::ContextValue::SYMBOL(Rc::downgrade(parent)),
            );
        }

        let eval_weaks = crate::core::symbols::symbol::Symbol::follow_ref(
            &eval_symbol,
            session,
            &mut context,
            true,
            false,
            None,
        );

        for eval_weak in eval_weaks.iter() {
            if let Some(field_class) = eval_weak.upgrade_weak() {
                if field_class.borrow().is_field_class(session) {
                    let field_type = field_class.borrow().name().to_string();

                    // For relational fields, show comodel name if available
                    if ["Many2one", "One2many", "Many2many"].contains(&field_type.as_str()) {
                        if let Some(comodel_value) = eval_weak.as_weak().context.get("comodel_name") {
                            let comodel = comodel_value.as_string();
                            return Some(format!("({}) {}", comodel, field_type));
                        }
                    }

                    return Some(field_type);
                }
            }
        }
    }

    None
}

impl XmlCompletionFeature {
    pub fn autocomplete(
        session: &mut SessionInfo,
        file_symbol: &Rc<RefCell<Symbol>>,
        file_info: &Rc<RefCell<FileInfo>>,
        line: u32,
        character: u32,
    ) -> Option<CompletionResponse> {
        let offset = file_info
            .borrow()
            .position_to_offset(line, character, session.sync_odoo.encoding);
        // content is not a field of FileInfo, but accessible via text_document in FileInfoAst or via file_mgr
        // For simplicity, we can get it from the file_mgr
        let file_info_ast = file_info.borrow().file_info_ast.clone();
        let content_opt = file_info_ast.borrow().text_document.clone();
        
        let content = match content_opt {
            Some(c) => c,
            None => return None,
        };
        
        // roxmltree requires the full document to parse
        let opt = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        };
        
        let doc = match roxmltree::Document::parse_with_options(content.contents(), opt) {
            Ok(doc) => doc,
            Err(_) => return None, // If XML is invalid, we can't do much yet
            // TODO: In the future, use a tolerant parser or try to fix the XML for completion
        };

        let root = doc.root();
        
        // Use XmlAstUtils to traverse and find context
        // We set on_dep_only to true to get symbols from dependencies as well
        let (ast_results, _) = XmlAstUtils::get_symbols(
            session,
            file_symbol,
            root,
            offset,
            true, 
        );

        let mut items = Vec::new();

        for result in ast_results {
            match result {
                XmlAstResult::SYMBOL(sym) => {
                    let sym_ref = sym.borrow();
                    let label = sym_ref.name().to_string();
                    let field_type = get_field_type(session, &sym);

                    items.push(CompletionItem {
                        label: label.clone(),
                        kind: Some(match sym_ref.typ() {
                            crate::constants::SymType::CLASS => CompletionItemKind::CLASS,
                            crate::constants::SymType::VARIABLE => CompletionItemKind::FIELD,
                            _ => CompletionItemKind::TEXT,
                        }),
                        label_details: if field_type.is_some() {
                            Some(CompletionItemLabelDetails {
                                detail: None,
                                description: field_type,
                            })
                        } else {
                            None
                        },
                        detail: None,
                        ..Default::default()
                    });
                }
                _ => {}
            }
        }

        if items.is_empty() {
            None
        } else {
            Some(CompletionResponse::List(CompletionList {
                is_incomplete: false,
                items,
            }))
        }
    }
}

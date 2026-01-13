use std::{cell::RefCell, rc::Rc};
use lsp_types::{CompletionItem, CompletionItemKind, CompletionList, CompletionResponse};

use crate::{
    core::{
        file_mgr::FileInfo,
        symbols::symbol::Symbol,
    },
    features::xml_ast_utils::{XmlAstResult, XmlAstUtils},
    threads::SessionInfo,
};

pub struct XmlCompletionFeature;

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
                    items.push(CompletionItem {
                        label: sym_ref.name().to_string(),
                        kind: Some(match sym_ref.typ() {
                            crate::constants::SymType::CLASS => CompletionItemKind::CLASS,
                            crate::constants::SymType::VARIABLE => CompletionItemKind::FIELD,
                            _ => CompletionItemKind::TEXT,
                        }),
                        detail: Some(format!("{:?}", sym_ref.typ())), // Simple detail for now
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

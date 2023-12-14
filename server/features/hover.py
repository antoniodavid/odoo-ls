import os

from ..features.parso_utils import ParsoUtils
from ..constants import *
from ..core.file_mgr import FileMgr
from ..core.symbol import ImportSymbol
from lsprotocol.types import (Hover, MarkupContent, MarkupKind)

class HoverFeature:

    @staticmethod
    def get_Hover(fileSymbol, parsoTree, line, character):

        symbol, effective_sym, factory, range, context = ParsoUtils.get_symbols(fileSymbol, parsoTree, line, character)

        if not symbol:
            return None
        if isinstance(symbol, str):
            return Hover(symbol)
        if isinstance(symbol, list):
            symbol = symbol[0]
        return HoverFeature._build_hover(symbol, effective_sym, factory, range, context)

    @staticmethod
    def build_markdown_description(symbol, effective_sym, factory, context):

        def build_block_1(symbol, type, infered_type):
            value =  "```python  \n"
            value += "(" + type + ") "
            if symbol.type == SymType.FUNCTION and not symbol.is_property:
                value += "def "
            value += symbol.name
            if symbol.type == SymType.FUNCTION and not symbol.is_property and symbol.ast_node:
                value += "(  \n" + ",  \n".join(arg.arg for arg in symbol.ast_node.args.args) + "  \n)"
            if infered_type and type != "module":
                if symbol.type == SymType.FUNCTION and not symbol.is_property:
                    value += " -> " + infered_type
                elif symbol.name != infered_type and symbol.type != SymType.CLASS:
                    if type == "type alias":
                        value += ": type[" + infered_type + "]"
                    else:
                        value += ": " + infered_type
            value += "  \n```"
            return value

        type_ref, next_context = symbol.next_ref(context)
        infered_type = "Any"
        if type_ref:
            type_ref.follow_ref(next_context, stop_on_type=True)[0]
            if type_ref != symbol:
                infered_type = type_ref.name
        #override infered_type if the effective_sym is built by a __get__
        if factory and effective_sym:
            infered_type = effective_sym.follow_ref(context, stop_on_type=True)[0].name
        type = str(symbol.type).lower()
        if symbol.eval and not symbol.eval.instance and not isinstance(symbol, ImportSymbol):
            type = "type alias"
        if symbol.type == SymType.FUNCTION:
            if symbol.is_property:
                type = "property"
            else:
                type = "method"
        #BLOCK 1: (type) **name** -> infered_type
        value = build_block_1(symbol, type, infered_type)
        #BLOCK 3: useful links:
        if infered_type not in ["Any", "constant"]:
            paths = type_ref.get_paths()
            if paths:
                path = FileMgr.pathname2uri(paths[0])
                if type_ref.type == SymType.PACKAGE:
                    path = os.path.join(path, "__init__.py")
                value += "  \n***  \n"
                value += "See also: " + "[" + type_ref.name + "](" + path + "#" + str(type_ref.start_pos[0]) + ")" + "  \n"
        #BLOCK 4: doc
        if symbol.doc:
            value += "  \n***  \n" + symbol.doc.value
        #if infered_type:
        #    value += "  \n-  \n**" + infered_type[2:] + "** : " + class_doc
        if symbol.name == "tomate" and symbol.type == SymType.VARIABLE: #easter egg (private joke)
            value = "Please rename your variable. Tomate is not a good name for a variable. You won't know what it means in 2 weeks (or even earlier)"
        return MarkupContent(
            kind=MarkupKind.Markdown,
            value=value
        )

    @staticmethod
    def _build_hover(symbol, effective_sym, factory, range, context):
        return Hover(
            contents=HoverFeature.build_markdown_description(symbol, effective_sym, factory, context),
            range=range
        )
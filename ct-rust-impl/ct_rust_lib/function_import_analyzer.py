from pathlib import Path
from tree_sitter import Language, Parser
from ct_rust_lib.tree_sitter_builder import LIB_PATH
from .logger import logger
from .models import FunctionImportAnalysis
from .node_type_finder import find_node_types

def analyze_function_imports(file_path: str) -> FunctionImportAnalysis:
    """
    Analyze which imports are used by each function in a Rust file.

    Args:
        file_path (str): Path to the Rust file.

    Returns:
        FunctionImportAnalysis: Analysis result containing function imports and unknown imports.
    """
    logger.debug(f"Analyzing function imports for file: {file_path}")
    code = read_file(file_path)
    root = parse_code(code)



    symbol_to_import = extract_use_declarations(root, code)
    debug_print_node_types_to_find(symbol_to_import, file_path)
    functions = extract_functions(root, code)
    function_imports = map_identifiers_to_imports(functions, symbol_to_import, code)
    unknown_imports = identify_unknown_imports(symbol_to_import, function_imports)

    logger.debug("Completed analysis of function imports.")
    return FunctionImportAnalysis(function_imports=function_imports, unknown_imports=unknown_imports)

def debug_print_node_types_to_find(symbol_to_import: dict, file_path: str):
    """
    Debug function to print node types for each symbol in use declarations.

    Args:
        symbol_to_import (dict): Mapping from symbol names to their import statements.
        file_path (str): Path to the Rust file.
    """
    logger.debug("Debugging node types for each symbol in use declarations:")
    for symbol in symbol_to_import.keys():
        node_types = find_node_types(file_path, symbol)
        if node_types:
            for node_type in node_types:
                logger.debug(f"{symbol}: {node_type}")
        else:
            logger.debug(f"{symbol}: No node types found")


def read_file(file_path: str) -> str:
    """
    Read the content of the given file.

    Args:
        file_path (str): Path to the file.

    Returns:
        str: Content of the file.
    """
    logger.debug(f"Reading file: {file_path}")
    with open(file_path, "r") as f:
        code = f.read()
    logger.debug("File read successfully.")
    return code

def parse_code(code: str):
    """
    Parse the Rust source code using Tree-sitter.

    Args:
        code (str): Rust source code.

    Returns:
        tree_sitter.Node: Root node of the parsed syntax tree.
    """
    logger.debug("Parsing code using Tree-sitter.")
    RUST_LANGUAGE = Language(str(LIB_PATH), "rust")
    parser = Parser()
    parser.set_language(RUST_LANGUAGE)
    tree = parser.parse(bytes(code, "utf8"))
    logger.debug("Code parsed successfully.")
    return tree.root_node

def extract_use_declarations(root, code: str) -> dict:
    """
    Extract all use declarations and map symbols to their imports.

    Args:
        root (tree_sitter.Node): Root node of the syntax tree.
        code (str): Rust source code.

    Returns:
        dict: A mapping from symbol names to their import statements.
    """
    logger.debug("Extracting use declarations.")
    symbol_to_import = {}

    def extract_use_declarations_recursive(node):
        if node.type != "use_declaration":
            for child in node.children:
                extract_use_declarations_recursive(child)
            return

        import_text = code[node.start_byte:node.end_byte].strip()
        logger.debug(f"Found use declaration: {import_text}")

        def extract_symbols(n):
            if n.type == "path":
                path_text = code[n.start_byte:n.end_byte].strip()
                symbol = path_text.split("::")[-1]
                symbol_to_import[symbol] = import_text
                logger.debug(f"Mapped symbol '{symbol}' to import '{import_text}'")
            elif n.type in ["use_tree", "use_list", "use_group"]:
                for child in n.named_children:
                    extract_symbols(child)
            elif n.type == "identifier":
                symbol = code[n.start_byte:n.end_byte].strip()
                symbol_to_import[symbol] = import_text
                logger.debug(f"Mapped symbol '{symbol}' to import '{import_text}'")
            elif n.type == "alias":
                alias = None
                for alias_child in n.children:
                    if alias_child.type == "identifier":
                        alias = code[alias_child.start_byte:alias_child.end_byte].strip()
                        break
                if alias:
                    symbol_to_import[alias] = import_text
                    logger.debug(f"Mapped alias '{alias}' to import '{import_text}'")
            else:
                for child in n.children:
                    extract_symbols(child)

        for child in node.children:
            extract_symbols(child)

    extract_use_declarations_recursive(root)

    logger.debug("Symbol to Import Mapping:")
    for symbol, imp in symbol_to_import.items():
        logger.debug(f"{symbol} -> {imp}")

    return symbol_to_import

def extract_functions(root, code: str) -> dict:
    """
    Extract function definitions and collect used identifiers.

    Args:
        root (tree_sitter.Node): Root node of the syntax tree.
        code (str): Rust source code.

    Returns:
        dict: A mapping from function names to sets of used identifiers.
    """
    logger.debug("Extracting functions.")
    functions = {}

    def extract_functions_recursive(node):
        if node.type != "function_item":
            for child in node.children:
                extract_functions_recursive(child)
            return

        function_name = None
        for child in node.children:
            if child.type == "identifier":
                function_name = code[child.start_byte:child.end_byte]
                break

        if function_name:
            functions[function_name] = set()
            logger.debug(f"Processing function: {function_name}")
            for child in node.children:
                if child.type in ["parameters", "block", "return_type"]:
                    collect_identifiers(child, functions[function_name], code)

    extract_functions_recursive(root)

    logger.debug("Functions and Identifiers:")
    for func, idents in functions.items():
        logger.debug(f"{func}: {idents}")

    return functions


def collect_identifiers(node, identifier_set: set, code: str):
    if node.type == "identifier":
        identifier = code[node.start_byte:node.end_byte]
        identifier_set.add(identifier)
        logger.debug(f"Collected identifier: {identifier}")
    elif node.type in ["type_reference", "trait_bounds", "higher_ranked_trait_bound", "impl_trait"]:
        logger.debug(f"Processing type-related node: {node.type}")
        collect_type_identifiers(node, identifier_set, code)
    else:
        for child in node.children:
            collect_identifiers(child, identifier_set, code)


def collect_type_identifiers(node, identifier_set: set, code: str):
    if node.type in ["path", "scoped_type_identifier"]:
        path_text = code[node.start_byte:node.end_byte].strip()
        symbols = path_text.split("::")
        logger.debug(f"Processing path: {path_text}")
        for symbol in symbols:
            if symbol:  # Ensure symbol is not empty
                identifier_set.add(symbol)
                logger.debug(f"Collected type identifier: {symbol}")
    elif node.type in ["generic_type", "impl_trait"]:
        logger.debug(f"Processing type-related node: {node.type}")
        for child in node.children:
            collect_type_identifiers(child, identifier_set, code)
    else:
        for child in node.children:
            collect_type_identifiers(child, identifier_set, code)


def map_identifiers_to_imports(functions: dict, symbol_to_import: dict, code: str) -> dict:
    logger.debug("Mapping identifiers to imports.")
    function_imports = {}

    # Expand grouped imports in symbol_to_import
    expanded_imports = {}
    for symbol, imp in symbol_to_import.items():
        if "{" in imp:  # Handle grouped imports
            base_import = imp.split("{")[0].strip() + "{};"
            group_items = imp.split("{")[1].split("}")[0].split(",")
            for item in group_items:
                item = item.strip()
                expanded_imports[item] = base_import.format(item)
        else:
            expanded_imports[symbol] = imp

    for func, idents in functions.items():
        function_imports[func] = set()
        for ident in idents:
            if ident in expanded_imports:
                function_imports[func].add(expanded_imports[ident])
                logger.debug(f"Function '{func}' uses import '{expanded_imports[ident]}' via '{ident}'")
            elif "::" in ident:
                base_ident = ident.split("::")[0]
                if base_ident in expanded_imports:
                    function_imports[func].add(expanded_imports[base_ident])
                    logger.debug(f"Function '{func}' uses import '{expanded_imports[base_ident]}' via '{base_ident}'")

        # Sort imports for each function
        function_imports[func] = sorted(function_imports[func])

    logger.debug("Function Imports:")
    for func, imps in function_imports.items():
        logger.debug(f"{func}: {imps}")

    return function_imports


def identify_unknown_imports(symbol_to_import: dict, function_imports: dict) -> list:
    logger.debug("Identifying unknown imports.")
    all_used_imports = set()
    for imp_set in function_imports.values():
        all_used_imports.update(imp_set)

    all_imports = set(symbol_to_import.values())
    unknown_imports = list(all_imports - all_used_imports)

    logger.debug("Unknown Imports:")
    for imp in unknown_imports:
        logger.debug(f"{imp}")

    return sorted(unknown_imports)

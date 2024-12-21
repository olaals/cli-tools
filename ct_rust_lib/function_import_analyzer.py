# ct_rust_lib/function_import_analyzer.py

from pathlib import Path
from tree_sitter import Language, Parser
from ct_rust_lib.tree_sitter_builder import LIB_PATH
import logging

# Optional: Enable debug logging
# To enable, uncomment the following lines
# logging.basicConfig(level=logging.DEBUG)
# logger = logging.getLogger(__name__)


def analyze_function_imports(file_path: str):
    """
    Analyze which imports are used by each function in a Rust file.

    Args:
        file_path (str): Path to the Rust file.

    Returns:
        tuple:
            dict: A dictionary where keys are function names, and values are lists of imports.
            list: A list of imports that could not be associated with any function (unknown).
    """
    code = read_file(file_path)
    root = parse_code(code)
    
    symbol_to_import = extract_use_declarations(root, code)
    functions = extract_functions(root, code)
    function_imports = map_identifiers_to_imports(functions, symbol_to_import, code)
    unknown_imports = identify_unknown_imports(symbol_to_import, function_imports)

    return function_imports, unknown_imports


def read_file(file_path: str) -> str:
    """
    Read the content of the given file.

    Args:
        file_path (str): Path to the file.

    Returns:
        str: Content of the file.
    """
    with open(file_path, "r") as f:
        code = f.read()
    return code


def parse_code(code: str):
    """
    Parse the Rust source code using Tree-sitter.

    Args:
        code (str): Rust source code.

    Returns:
        tree_sitter.Tree: Parsed syntax tree.
    """
    RUST_LANGUAGE = Language(str(LIB_PATH), "rust")
    parser = Parser()
    parser.set_language(RUST_LANGUAGE)
    tree = parser.parse(bytes(code, "utf8"))
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
    symbol_to_import = {}

    def extract_use_declarations_recursive(node):
        if node.type != "use_declaration":
            for child in node.children:
                extract_use_declarations_recursive(child)
            return

        # Extract the full import statement
        import_text = code[node.start_byte:node.end_byte].strip()
        print(f"Found use declaration: {import_text}")

        # Recursive function to extract symbols from use_tree
        def extract_symbols(n):
            if n.type == "path":
                # For simple use paths, the symbol is the last identifier
                path_text = code[n.start_byte:n.end_byte].strip()
                symbol = path_text.split("::")[-1]
                symbol_to_import[symbol] = import_text
                print(f"Mapped symbol '{symbol}' to import '{import_text}'")
            elif n.type in ["use_tree", "use_list", "use_group"]:
                for child in n.named_children:
                    extract_symbols(child)
            elif n.type == "identifier":
                symbol = code[n.start_byte:n.end_byte].strip()
                symbol_to_import[symbol] = import_text
                print(f"Mapped symbol '{symbol}' to import '{import_text}'")
            elif n.type == "alias":
                # Handle aliasing: use foo::bar as baz;
                alias = None
                for alias_child in n.children:
                    if alias_child.type == "identifier":
                        alias = code[alias_child.start_byte:alias_child.end_byte].strip()
                        break
                if alias:
                    symbol_to_import[alias] = import_text
                    print(f"Mapped alias '{alias}' to import '{import_text}'")
            else:
                for child in n.children:
                    extract_symbols(child)

        # Start extracting symbols from the use_declaration node
        for child in node.children:
            extract_symbols(child)

    extract_use_declarations_recursive(root)

    # Debug: Print symbol_to_import mapping
    print("\nSymbol to Import Mapping:")
    for symbol, imp in symbol_to_import.items():
        print(f"{symbol} -> {imp}")

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
    functions = {}

    def extract_functions_recursive(node):
        if node.type != "function_item":
            for child in node.children:
                extract_functions_recursive(child)
            return

        # Extract function name
        function_name = None
        for child in node.children:
            if child.type == "identifier":
                function_name = code[child.start_byte:child.end_byte]
                break

        if function_name:
            functions[function_name] = set()
            print(f"\nProcessing function: {function_name}")
            # Traverse the function body and parameters to collect identifiers
            for child in node.children:
                if child.type in ["parameters", "block", "return_type"]:
                    collect_identifiers(child, functions[function_name], code)

    extract_functions_recursive(root)

    # Debug: Print functions and their identifiers
    print("\nFunctions and Identifiers:")
    for func, idents in functions.items():
        print(f"{func}: {idents}")

    return functions


def collect_identifiers(node, identifier_set: set, code: str):
    """
    Collect all identifiers used in a given node and add them to the identifier_set.

    Args:
        node (tree_sitter.Node): The syntax tree node to traverse.
        identifier_set (set): Set to store collected identifiers.
        code (str): Rust source code.
    """
    if node.type == "identifier":
        identifier = code[node.start_byte:node.end_byte]
        identifier_set.add(identifier)
        print(f"Collected identifier: {identifier}")
    elif node.type == "type_reference":
        collect_type_identifiers(node, identifier_set, code)
    else:
        for child in node.children:
            collect_identifiers(child, identifier_set, code)


def collect_type_identifiers(node, identifier_set: set, code: str):
    """
    Specifically handle type paths to extract their identifiers.

    Args:
        node (tree_sitter.Node): The syntax tree node to traverse.
        identifier_set (set): Set to store collected type identifiers.
        code (str): Rust source code.
    """
    if node.type == "path":
        # Split the path and add only the last identifier
        path_text = code[node.start_byte:node.end_byte].strip()
        symbols = path_text.split("::")
        if symbols:
            symbol = symbols[-1]
            identifier_set.add(symbol)
            print(f"Collected type identifier: {symbol}")
    else:
        for child in node.children:
            collect_type_identifiers(child, identifier_set, code)


def map_identifiers_to_imports(functions: dict, symbol_to_import: dict, code: str) -> dict:
    """
    Map collected identifiers to their corresponding imports.

    Args:
        functions (dict): Mapping from function names to sets of identifiers.
        symbol_to_import (dict): Mapping from symbols to import statements.
        code (str): Rust source code.

    Returns:
        dict: Mapping from function names to sets of import statements.
    """
    function_imports = {}
    for func, idents in functions.items():
        function_imports[func] = set()
        for ident in idents:
            if ident in symbol_to_import:
                function_imports[func].add(symbol_to_import[ident])
                print(f"Function '{func}' uses import '{symbol_to_import[ident]}' via '{ident}'")
            # Additionally, handle cases where identifiers are used with paths, e.g., Regex::new
            elif "::" in ident:
                base_ident = ident.split("::")[0]
                if base_ident in symbol_to_import:
                    function_imports[func].add(symbol_to_import[base_ident])
                    print(f"Function '{func}' uses import '{symbol_to_import[base_ident]}' via '{base_ident}'")
    # Debug: Print function_imports mapping
    print("\nFunction Imports:")
    for func, imps in function_imports.items():
        print(f"{func}: {imps}")
    # Convert sets to sorted lists for consistency
    function_imports = {k: sorted(v) for k, v in function_imports.items()}
    return function_imports


def identify_unknown_imports(symbol_to_import: dict, function_imports: dict) -> list:
    """
    Identify imports that are not associated with any function.

    Args:
        symbol_to_import (dict): Mapping from symbols to import statements.
        function_imports (dict): Mapping from function names to import statements.

    Returns:
        list: List of unknown import statements.
    """
    all_used_imports = set()
    for imp_set in function_imports.values():
        all_used_imports.update(imp_set)

    all_imports = set(symbol_to_import.values())
    unknown_imports = list(all_imports - all_used_imports)

    # Debug: Print unknown imports
    print("\nUnknown Imports:")
    for imp in unknown_imports:
        print(f"{imp}")

    return sorted(unknown_imports)


# Optional: Pytest tests can be added below if desired.
# Ensure that tests are within the `if __name__ == "__main__":` block to prevent execution during imports.

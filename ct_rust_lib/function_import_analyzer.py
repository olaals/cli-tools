# ct_rust_lib/function_import_analyzer.py

from pathlib import Path
from tree_sitter import Language, Parser
from ct_rust_lib.tree_sitter_builder import LIB_PATH


def analyze_function_imports(file_path: str):
    """
    Analyze which imports are used by each function in a Rust file.

    Args:
        file_path (str): Path to the Rust file.

    Returns:
        dict: A dictionary where keys are function names, and values are lists of imports.
        list: A list of imports that could not be associated with any function (unknown).
    """
    # Initialize Tree-sitter parser for Rust
    RUST_LANGUAGE = Language(str(LIB_PATH), "rust")
    parser = Parser()
    parser.set_language(RUST_LANGUAGE)

    # Read the Rust source code
    with open(file_path, "r") as f:
        code = f.read()

    tree = parser.parse(bytes(code, "utf8"))
    root = tree.root_node

    symbol_to_import = {}  # symbol -> import statement
    functions = {}          # function name -> set of used symbols

    # Step 1: Extract all use declarations and map symbols to their imports
    def extract_use_declarations(node):
        nonlocal symbol_to_import
        if node.type != "use_declaration":
            for child in node.children:
                extract_use_declarations(child)
            return

        # Extract the full import statement
        import_text = code[node.start_byte:node.end_byte].strip()

        # Recursive function to extract symbols from use_tree
        def extract_symbols(n):
            if n.type == "path":
                # For simple use paths, the symbol is the last identifier
                path_text = code[n.start_byte:n.end_byte].strip()
                symbol = path_text.split("::")[-1]
                symbol_to_import[symbol] = import_text
            elif n.type == "use_tree":
                for child in n.children:
                    extract_symbols(child)
            elif n.type == "use_list":
                for child in n.named_children:
                    extract_symbols(child)
            elif n.type == "use_group":
                for child in n.named_children:
                    extract_symbols(child)
            elif n.type == "identifier":
                symbol = code[n.start_byte:n.end_byte].strip()
                symbol_to_import[symbol] = import_text
            elif n.type == "alias":
                # Handle aliasing: use foo::bar as baz;
                # Here, 'baz' is the symbol
                alias = None
                for alias_child in n.children:
                    if alias_child.type == "identifier":
                        alias = code[alias_child.start_byte:alias_child.end_byte].strip()
                if alias:
                    symbol_to_import[alias] = import_text
            else:
                for child in n.children:
                    extract_symbols(child)

        # For each use_declaration, parse its children
        for child in node.children:
            extract_symbols(child)

    extract_use_declarations(root)

    # Step 2: Extract function definitions and collect used identifiers
    def extract_functions(node):
        nonlocal functions
        if node.type != "function_item":
            for child in node.children:
                extract_functions(child)
            return

        # Extract function name
        function_name = None
        for child in node.children:
            if child.type == "identifier":
                function_name = code[child.start_byte:child.end_byte]
                break

        if function_name:
            functions[function_name] = set()
            # Traverse the function body to collect identifiers
            for child in node.children:
                if child.type in ["parameters", "block"]:
                    collect_identifiers(child, functions[function_name])

    # Collect all identifiers used in the function body
    def collect_identifiers(node, identifier_set):
        if node.type == "identifier":
            identifier = code[node.start_byte:node.end_byte]
            identifier_set.add(identifier)
        else:
            for child in node.children:
                collect_identifiers(child, identifier_set)

    extract_functions(root)

    # Step 3: Map identifiers to imports
    function_imports = {}
    for func, idents in functions.items():
        function_imports[func] = set()
        for ident in idents:
            if ident in symbol_to_import:
                function_imports[func].add(symbol_to_import[ident])
            # Additionally, handle cases where identifiers are used with paths, e.g., Regex::new
            # For this, consider the base identifier before '::'
            elif "::" in ident:
                base_ident = ident.split("::")[0]
                if base_ident in symbol_to_import:
                    function_imports[func].add(symbol_to_import[base_ident])

    # Step 4: Identify unknown imports
    all_used_imports = set()
    for imp_set in function_imports.values():
        all_used_imports.update(imp_set)

    all_imports = set(symbol_to_import.values())
    unknown_imports = list(all_imports - all_used_imports)

    # Convert sets to sorted lists for consistency
    function_imports = {k: sorted(v) for k, v in function_imports.items()}

    return function_imports, sorted(unknown_imports)

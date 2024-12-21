from pathlib import Path
from tree_sitter import Parser, Language
from ct_rust_lib.tree_sitter_builder import LIB_PATH

def extract_functions(file_path: str, pub_only: bool = True):
    """
    Extract function names from a Rust file using tree-sitter.

    Args:
        file_path (str): Path to the Rust file.
        pub_only (bool): If True, include only public functions.

    Returns:
        A list of function names.
    """
    RUST_LANGUAGE = Language(str(LIB_PATH), "rust")
    parser = Parser()
    parser.set_language(RUST_LANGUAGE)

    with open(file_path, "r") as f:
        code = f.read()

    tree = parser.parse(bytes(code, "utf8"))
    return _extract_functions_from_syntax_tree(tree, code, pub_only)

def _extract_functions_from_syntax_tree(tree, code, pub_only):
    """
    Traverse the syntax tree to extract function names.

    Args:
        tree: The parsed syntax tree.
        code: The source code as a string.
        pub_only: If True, include only public functions.

    Returns:
        A list of function names.
    """
    functions = []

    def traverse(node):
        if node.type == "function_item":
            # Extract function visibility
            visibility = None
            for child in node.children:
                if child.type == "visibility_modifier":
                    visibility = code[child.start_byte:child.end_byte]
                elif child.type == "identifier":
                    name = code[child.start_byte:child.end_byte]
                    if not pub_only or visibility == "pub":
                        functions.append(name)
        # Recurse into children
        for child in node.children:
            traverse(child)

    traverse(tree.root_node)
    return functions

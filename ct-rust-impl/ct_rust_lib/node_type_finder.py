# ct_rust_lib/node_type_finder.py

from pathlib import Path
from tree_sitter import Language, Parser
from .logger import logger
from ct_rust_lib.tree_sitter_builder import LIB_PATH

def find_node_types(file_path: str, target_name: str) -> list:
    """
    Find all node types in the Rust file that correspond to the given name.

    Args:
        file_path (str): Path to the Rust file.
        target_name (str): The name to search for (variable, trait, etc.).

    Returns:
        list: A list of node types where the name is found.
    """
    logger.debug(f"Finding node types for '{target_name}' in file: {file_path}")
    code = read_file(file_path)
    root = parse_code(code)

    matching_node_types = []

    def traverse(node):
        # Extract the text corresponding to the node
        node_text = code[node.start_byte:node.end_byte].strip()
        # Check if the node's text matches the target name
        if node_text == target_name:
            matching_node_types.append(node.type)
            logger.debug(f"Found match: {target_name} at node type '{node.type}'")
        # Recursively traverse child nodes
        for child in node.children:
            traverse(child)

    traverse(root)
    logger.debug(f"Total matches found: {len(matching_node_types)}")
    return matching_node_types


def read_file(file_path: str) -> str:
    """
    Read the content of the given file.

    Args:
        file_path (str): Path to the file.

    Returns:
        str: Content of the file.
    """
    logger.debug(f"Reading file: {file_path}")
    with open(file_path, "r", encoding="utf-8") as f:
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

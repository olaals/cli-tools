# ct_rust_lib/splitter.py

from pathlib import Path
from ct_rust_lib.function_import_analyzer import analyze_function_imports
from ct_rust_lib.function_processor import extract_functions
from ct_rust_lib.models import FunctionImportAnalysis
from typing import Dict, Set
import shutil
import os
import logging

# Initialize logger
logger = logging.getLogger("ct_rust")

def split_file(file_path: Path, output_dir: Path):
    """
    Split the given Rust file into separate files for each function,
    including only the necessary imports for each function.

    Args:
        file_path (Path): Path to the Rust file to split.
        output_dir (Path): Directory to place the split files.
    """
    analysis: FunctionImportAnalysis = analyze_function_imports(str(file_path))

    # Extract all functions
    functions = extract_functions(str(file_path), pub_only=False)  # Extract all functions

    # Read the entire file content
    with open(file_path, "r") as f:
        code = f.read()

    # Parse the code to get function AST nodes
    from tree_sitter import Parser, Language
    from ct_rust_lib.tree_sitter_builder import LIB_PATH

    RUST_LANGUAGE = Language(str(LIB_PATH), "rust")
    parser = Parser()
    parser.set_language(RUST_LANGUAGE)
    tree = parser.parse(bytes(code, "utf8"))
    root_node = tree.root_node

    # Ensure the output directory exists
    output_dir.mkdir(parents=True, exist_ok=True)
    logger.debug(f"Output directory '{output_dir}' is ready.")

    # Function to extract function code from AST
    def extract_function_code_from_node(node):
        return code[node.start_byte:node.end_byte]

    # Iterate over each function and create separate files
    for function_name in functions:
        # Get the function's imports and ensure it's a set
        imports = set(analysis.function_imports.get(function_name, []))

        # Add unknown imports
        unknown_imports = set(analysis.unknown_imports)
        imports.update(unknown_imports)

        # Extract the function's code
        function_node = find_function_node(root_node, function_name)
        if not function_node:
            logger.warning(f"Function '{function_name}' not found in AST.")
            continue

        function_code = extract_function_code_from_node(function_node)

        # Prepare the new file content
        import_code = "\n".join(sorted(imports))
        new_file_content = f"{import_code}\n\n{function_code}\n"

        # Write to the new file
        new_file_path = output_dir / f"{function_name}.rs"
        with open(new_file_path, "w") as nf:
            nf.write(new_file_content)

        logger.debug(f"Created split file: {new_file_path}")

def find_function_node(root_node, function_name: str):
    """
    Find the AST node for the given function name.

    Args:
        root_node: The root AST node.
        function_name (str): The name of the function to find.

    Returns:
        tree_sitter.Node or None: The function node if found, else None.
    """
    for node in root_node.named_children:
        if node.type == "function_item":
            for child in node.named_children:
                if child.type == "identifier":
                    name = child.text.decode('utf-8')
                    if name == function_name:
                        return node
    return None

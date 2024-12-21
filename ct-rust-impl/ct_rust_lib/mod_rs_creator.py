from pathlib import Path
from ct_rust_lib.function_processor import extract_functions
from ct_rust_lib.tree_sitter_builder import build_language_library


def generate_and_write_mod_rs(directory: Path):
    """
    Generate and write a mod.rs file that includes all .rs files in the directory
    and re-exports public functions.

    Args:
        directory (Path): Path to the directory containing .rs files.

    Returns:
        Path: The path to the written mod.rs file.
    """
    if not directory.is_dir():
        raise ValueError(f"{directory} is not a directory.")

    # Gather all .rs files except mod.rs
    rust_files = [f for f in directory.glob("*.rs") if f.name != "mod.rs"]
    mod_lines = []
    pub_use_lines = []

    build_language_library()

    for rust_file in rust_files:
        module_name = rust_file.stem
        mod_lines.append(f"mod {module_name};")
        functions = extract_functions(str(rust_file), pub_only=True)
        if functions:
            pub_use_lines.append(f"pub use {module_name}::{{{', '.join(functions)}}};")

    # Combine the lines for mod.rs
    mod_rs_content = "\n".join(mod_lines + [""] + pub_use_lines)

    # Write to mod.rs
    mod_file_path = directory / "mod.rs"
    with mod_file_path.open("w") as mod_file:
        mod_file.write(mod_rs_content)

    return mod_file_path

from pathlib import Path
from typing import List
import re

def update_imports_in_directory(directory: Path, old_import: str, new_import: str) -> List[Path]:
    """
    Replace all instances of an old import with a new one across all .rs files in a directory.

    Args:
        directory (Path): Path to the directory containing .rs files.
        old_import (str): The old import statement to replace.
        new_import (str): The new import statement to replace with.

    Returns:
        List[Path]: A list of files that were updated.
    """
    if not directory.is_dir():
        raise ValueError(f"{directory} is not a directory.")

    updated_files = []

    for rust_file in directory.glob("*.rs"):
        with open(rust_file, "r") as f:
            content = f.read()

        updated_content = re.sub(rf"\b{re.escape(old_import)}\b", new_import, content)

        if updated_content != content:
            with open(rust_file, "w") as f:
                f.write(updated_content)
            updated_files.append(rust_file)

    return updated_files

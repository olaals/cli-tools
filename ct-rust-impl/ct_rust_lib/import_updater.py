from pathlib import Path
from typing import List
import re
from .logger import logger


def read_file(file_path: Path) -> str:
    with open(file_path, "r") as f:
        return f.read()


def write_file(file_path: Path, content: str) -> None:
    with open(file_path, "w") as f:
        f.write(content)


def update_imports_in_content(content: str, old_import: str, new_import: str) -> str:
    return re.sub(rf"\b{re.escape(old_import)}\b", new_import, content)


def update_imports_in_directory(directory: Path, old_import: str, new_import: str) -> List[Path]:
    if not directory.is_dir():
        raise ValueError(f"{directory} is not a directory.")

    updated_files = []
    logger.debug(f"Updating imports in directory: {directory}")
    logger.info(f"Updating imports from {old_import} to {new_import}")

    for rust_file in directory.glob("*.rs"):
        print(f"Processing file: {rust_file}")  # Debug
        content = read_file(rust_file)
        updated_content = re.sub(re.escape(old_import), new_import, content)

        if updated_content != content:
            write_file(rust_file, updated_content)
            updated_files.append(rust_file)
            print(f"Updated file: {rust_file}")  # Debug

    print(f"Updated files: {updated_files}")  # Debug
    return updated_files



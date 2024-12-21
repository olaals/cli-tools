

1: Prefer splitting up a function does io (reading writing) to a function that first does io 
and then do the main processing in its own function and returns the result

For example dont do this:

```python

from pathlib import Path
from typing import List
import re

def update_imports_in_directory(directory: Path, old_import: str, new_import: str) -> List[Path]:
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
```

but rather this:
```python
from pathlib import Path
from typing import List
import re


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

    for rust_file in directory.glob("*.rs"):
        content = read_file(rust_file)
        updated_content = update_imports_in_content(content, old_import, new_import)

        if updated_content != content:
            write_file(rust_file, updated_content)
            updated_files.append(rust_file)

    return updated_files

```

This makes it easier to write tests for process_file_func,
since we can test the main logic without having to mock the file io.


2: Prefer type definitions


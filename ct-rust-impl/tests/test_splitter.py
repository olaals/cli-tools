# tests/test_splitter.py

import pytest
from pathlib import Path
from ct_rust_lib.splitter import split_file

def test_split_file(tmp_path, logger):
    """
    Test the split_file function to ensure that a Rust file is correctly split
    into separate files for each function with appropriate imports.
    """
    # Sample Rust code with multiple functions and imports
    rust_code = """
    use crate::foo::Foo;
    use crate::bar::Bar;
    use crate::common::CommonStruct;

    pub fn foo_function() -> Foo {
        // Implementation of foo_function
    }

    fn bar_function(param: Bar) -> CommonStruct {
        // Implementation of bar_function
    }

    pub fn common_function() -> CommonStruct {
        // Implementation of common_function
    }
    """

    # Create a temporary Rust file
    rust_file = tmp_path / "test.rs"
    rust_file.write_text(rust_code)

    # Define the output directory for split files
    output_dir = tmp_path / "split_files"

    # Invoke the split_file function
    split_file(rust_file, output_dir)

    # Assertions

    # 1. Check that the output directory exists
    assert output_dir.exists(), "Output directory was not created."

    # 2. Define expected split files
    expected_files = {
        "foo_function.rs": {
            "imports": ["use crate::foo::Foo;"],
            "function_def": "pub fn foo_function() -> Foo",
        },
        "bar_function.rs": {
            "imports": ["use crate::bar::Bar;", "use crate::common::CommonStruct;"],
            "function_def": "fn bar_function(param: Bar) -> CommonStruct",
        },
        "common_function.rs": {
            "imports": ["use crate::common::CommonStruct;"],
            "function_def": "pub fn common_function() -> CommonStruct",
        },
    }

    # 3. Iterate through expected files and verify their existence and content
    for file_name, contents in expected_files.items():
        split_file_path = output_dir / file_name
        assert split_file_path.exists(), f"{file_name} was not created."

        file_content = split_file_path.read_text()

        # Check for the presence of necessary imports
        for imp in contents["imports"]:
            assert imp in file_content, f"Import '{imp}' not found in {file_name}."

        # Check for the presence of the function definition
        assert contents["function_def"] in file_content, f"Function '{contents['function_def']}' not found in {file_name}."

        # Optionally, ensure that no unnecessary imports are present
        # For example, 'use crate::common::CommonStruct;' should only appear where needed
        # This part can be expanded based on specific requirements

    # 4. Ensure no extra files are created
    actual_files = {file.name for file in output_dir.iterdir() if file.is_file()}
    expected_file_names = set(expected_files.keys())
    assert actual_files == expected_file_names, f"Unexpected files found: {actual_files - expected_file_names}"

    # Logging for debugging purposes
    logger.debug("All split files created correctly with appropriate imports and function definitions.")


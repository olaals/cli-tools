# tests/test_function_import_analyzer.py

import pytest
from pathlib import Path
from ct_rust_lib.function_import_analyzer import analyze_function_imports
from ct_rust_lib.models import FunctionImportAnalysis

def test_detect_trait_imports(tmp_path, logger):
    """
    Test that traits used in function argument types, return types, and trait bounds are correctly detected and mapped.
    """
    rust_code = """
    use my_crate::traits::MyTrait;
    use std::fmt::Display;

    pub fn display_item(item: impl Display) {
        println!("{}", item);
    }

    pub fn process_trait<T: MyTrait>(item: T) {
        item.do_something();
    }

    pub fn combined(item: impl Display + MyTrait) {
        println!("{}", item);
        item.do_something();
    }

    pub fn unused_function() {
        // This function doesn't use any imports
    }
    """

    # Create a temporary Rust file with the above code
    rust_file = tmp_path / "test_trait_import.rs"
    rust_file.write_text(rust_code)

    logger.debug("Starting test for detect_trait_imports")

    # Analyze the Rust file
    analysis_result: FunctionImportAnalysis = analyze_function_imports(str(rust_file))

    # Define the expected mapping of functions to their imports
    expected_function_imports = {
        "display_item": ["use std::fmt::Display;"],
        "process_trait": ["use my_crate::traits::MyTrait;"],
        "combined": ["use std::fmt::Display;", "use my_crate::traits::MyTrait;"],
        "unused_function": [],
    }

    # Define the expected list of unknown imports
    expected_unknown_imports = []

    logger.debug(f"Expected Function Imports: {expected_function_imports}")
    logger.debug(f"Actual Function Imports: {analysis_result.function_imports}")

    # Sort the import lists for comparison to ensure order doesn't affect the test
    for func in expected_function_imports:
        expected_function_imports[func].sort()
    for func in analysis_result.function_imports:
        analysis_result.function_imports[func].sort()

    # Assert that the function_imports match the expected mapping
    assert analysis_result.function_imports == expected_function_imports, \
        f"Expected function imports {expected_function_imports}, but got {analysis_result.function_imports}"

    # Assert that there are no unknown imports
    assert analysis_result.unknown_imports == expected_unknown_imports, \
        f"Expected unknown imports {expected_unknown_imports}, but got {analysis_result.unknown_imports}"

    logger.debug("Test passed successfully")



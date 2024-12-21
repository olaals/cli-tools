# tests/test_function_import_analyzer.py

import pytest
from pathlib import Path
from ct_rust_lib.function_import_analyzer import analyze_function_imports
from ct_rust_lib.models import FunctionImportAnalysis

def test_detect_import_in_function_argument(tmp_path, logger):
    """
    Test that imports used in function argument types are correctly detected and mapped.
    """
    rust_code = """
    use my_crate::config::InputLinksConfig;
    use my_crate::utils::process_config;
    
    pub fn process_links(
        config: InputLinksConfig,
    ) {
        process_config(config);
    }
    
    pub fn unused_function() {
        // This function doesn't use any imports
    }
    """
    
    # Create a temporary Rust file with the above code
    rust_file = tmp_path / "test_arg_import.rs"
    rust_file.write_text(rust_code)
    
    logger.debug("Starting test for detect_import_in_function_argument")
    
    # Analyze the Rust file
    analysis_result: FunctionImportAnalysis = analyze_function_imports(str(rust_file))
    
    # Define the expected mapping of functions to their imports
    expected_function_imports = {
        "process_links": [
            "use my_crate::config::InputLinksConfig;",
            "use my_crate::utils::process_config;"
        ],
        "unused_function": [],
    }
    
    # Define the expected list of unknown imports
    expected_unknown_imports = []
    
    logger.debug(f"Expected Function Imports: {expected_function_imports}")
    logger.debug(f"Actual Function Imports: {analysis_result.function_imports}")
    
    # Assert that the function_imports match the expected mapping
    assert analysis_result.function_imports == expected_function_imports, \
        f"Expected function imports {expected_function_imports}, but got {analysis_result.function_imports}"
    
    # Assert that there are no unknown imports
    assert analysis_result.unknown_imports == expected_unknown_imports, \
        f"Expected unknown imports {expected_unknown_imports}, but got {analysis_result.unknown_imports}"
    
    logger.debug("Test passed successfully")

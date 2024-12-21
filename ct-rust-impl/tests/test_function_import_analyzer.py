import pytest
from pathlib import Path
from ct_rust_lib.function_import_analyzer import analyze_function_imports
from ct_rust_lib.models import FunctionImportAnalysis

def test_analyze_function_imports(tmp_path, logger):
    rust_code = """
    use std::collections::HashMap;
    use std::fmt::Debug;
    use my_crate::some_module::SomeStruct;
    use my_crate::another_module::{AnotherStruct, helper_function};
    use unused_crate::UnusedImport;

    pub fn my_function() -> SomeStruct {
        let mut map: HashMap<String, i32> = HashMap::new();
        map.insert("key".to_string(), 42);
        SomeStruct {}
    }

    pub fn another_function(param: AnotherStruct) -> impl Debug {
        helper_function();
        param
    }

    pub fn unused_function() {
        // This function doesn't use any imports
    }
    """

    rust_file = tmp_path / "test.rs"
    rust_file.write_text(rust_code)

    logger.debug("Starting test for analyze_function_imports")

    analysis_result: FunctionImportAnalysis = analyze_function_imports(str(rust_file))

    assert analysis_result.function_imports == {
        "my_function": ["use std::collections::HashMap;", "use my_crate::some_module::SomeStruct;"],
        "another_function": ["use my_crate::another_module::AnotherStruct;", "use my_crate::another_module::helper_function;", "use std::fmt::Debug;"],
        "unused_function": [],
    }

    assert analysis_result.unknown_imports == ["use unused_crate::UnusedImport;"]

    logger.debug("Test passed successfully")

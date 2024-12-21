import pytest
from pathlib import Path
from ct_rust_lib.import_updater import update_imports_in_directory


def test_update_imports_in_directory(tmp_path, logger):
    rust_code = """
    use old_crate::SomeStruct;

    pub fn my_function() -> SomeStruct {
        // ...
    }
    """
    rust_file = tmp_path / "test.rs"
    rust_file.write_text(rust_code)

    logger.debug("Starting test for update_imports_in_directory")

    updated_files = update_imports_in_directory(
        tmp_path,
        "use old_crate::SomeStruct;",
        "use new_crate::SomeStruct;",
    )

    assert rust_file in updated_files
    updated_content = rust_file.read_text()
    assert "use new_crate::SomeStruct;" in updated_content
    assert "use old_crate::SomeStruct;" not in updated_content

    logger.debug("Test passed successfully")

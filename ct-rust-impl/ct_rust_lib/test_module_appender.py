from pathlib import Path
import re
from .logger import logger

class TestModuleAppender:
    """
    A class to append an empty test module shell to a Rust .rs file.
    """

    TEST_MODULE_PATTERN = re.compile(r'(?m)^\s*#\s*\[cfg\s*\(\s*test\s*\)\s*\]')
    TEST_MODULE_TEMPLATE = """
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
"""

    def __init__(self, file_path: str):
        self.file_path = Path(file_path)
        if not self.file_path.is_file():
            raise ValueError(f"{self.file_path} is not a valid file.")

    def append_test_module(self) -> bool:
        """
        Appends the test module shell to the file if it doesn't already exist.
        
        Returns:
            bool: True if appended, False if already exists.
        """
        logger.debug(f"Appending test module to {self.file_path}")
        try:
            content = self.file_path.read_text()

            if self.TEST_MODULE_PATTERN.search(content):
                logger.info(f"Test module already exists in {self.file_path}")
                return False  # Test module already exists

            # Ensure there's at least one newline before appending
            if not content.endswith('\n'):
                content += '\n'

            content += self.TEST_MODULE_TEMPLATE
            self.file_path.write_text(content)
            logger.info(f"Appended test module to {self.file_path}")
            return True
        except Exception as e:
            logger.error(f"Failed to append test module: {e}")
            raise
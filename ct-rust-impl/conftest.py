import logging
import pytest


@pytest.fixture
def logger():
    """Fixture to provide a logger for tests."""
    logger = logging.getLogger("ct_rust")
    handler = logging.StreamHandler()
    formatter = logging.Formatter("%(asctime)s - %(levelname)s - %(message)s")
    handler.setFormatter(formatter)
    logger.addHandler(handler)
    logger.setLevel(logging.DEBUG)
    handler.setLevel(logging.DEBUG)
    yield logger
    logger.removeHandler(handler)


def pytest_runtest_makereport(item, call):
    """Hook to modify behavior based on test outcome."""
    if call.when == "call":  # Only process test function calls
        if call.excinfo is None:  # Test succeeded
            logging.getLogger("ct_rust_tests").setLevel(logging.CRITICAL)
        else:  # Test failed
            logging.getLogger("ct_rust_tests").setLevel(logging.DEBUG)

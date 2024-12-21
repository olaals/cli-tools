import logging


def setup_logger(name: str = "ct_rust", level: str = "INFO") -> logging.Logger:
    """
    Centralized logger setup for the project.
    """
    logger = logging.getLogger(name)

    # Avoid duplicate handlers
    if not logger.handlers:
        handler = logging.StreamHandler()
        formatter = logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
        handler.setFormatter(formatter)
        logger.addHandler(handler)

    logger.setLevel(getattr(logging, level.upper(), logging.INFO))
    return logger


logger = setup_logger()

import subprocess
from pathlib import Path
from tree_sitter import Language

SCRIPT_DIR = Path(__file__).parent.parent.resolve()
TREE_SITTER_BUILDS_DIR = SCRIPT_DIR / "tree-sitter-builds"
TREE_SITTER_RUST_DIR = TREE_SITTER_BUILDS_DIR / "tree-sitter-rust"
BUILD_DIR = SCRIPT_DIR / "build"
LIB_PATH = BUILD_DIR / "my-languages.so"
TREE_SITTER_REPO = "https://github.com/tree-sitter/tree-sitter-rust.git"

def ensure_tree_sitter_rust():
    """
    Ensure the tree-sitter-rust repository is cloned in the appropriate location.
    """
    if not TREE_SITTER_RUST_DIR.exists():
        print(f"Cloning {TREE_SITTER_REPO} into {TREE_SITTER_RUST_DIR}...")
        TREE_SITTER_BUILDS_DIR.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "clone", TREE_SITTER_REPO, str(TREE_SITTER_RUST_DIR)], check=True)
    else:
        print(f"{TREE_SITTER_RUST_DIR} already exists. Pulling latest changes...")
        subprocess.run(["git", "-C", str(TREE_SITTER_RUST_DIR), "pull"], check=True)

def build_language_library():
    """
    Build the tree-sitter shared library for Rust.
    """
    ensure_tree_sitter_rust()
    BUILD_DIR.mkdir(exist_ok=True)
    Language.build_library(
        str(LIB_PATH),
        [str(TREE_SITTER_RUST_DIR)]
    )
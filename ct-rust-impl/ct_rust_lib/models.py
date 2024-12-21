from dataclasses import dataclass
from typing import Dict, List

@dataclass
class FunctionImportAnalysis:
    function_imports: Dict[str, List[str]]
    unknown_imports: List[str]


#!/usr/bin/env python3

import argparse
import json
import os

def colored(text: str, color_code: str) -> str:
    return f"{color_code}{text}\033[0m"

def parse_bicep_file(file_path: str) -> None:
    if not os.path.exists(file_path):
        print(colored("File does not exist.", "\033[91m"))
        return

    os.system(f"bicep build {file_path} --outfile temp.json")

    if not os.path.exists("temp.json"):
        print(colored("Failed to generate ARM template.", "\033[91m"))
        return

    with open("temp.json", "r") as f:
        arm_template = json.load(f)

    os.remove("temp.json")

    parameters = arm_template.get("parameters", {})
    outputs = arm_template.get("outputs", {})

    print(colored("Inputs:", "\033[92m"))
    for param in parameters.keys():
        print(colored(f"  - {param}", "\033[96m"))

    print(colored("Outputs:", "\033[92m"))
    for output in outputs.keys():
        print(colored(f"  - {output}", "\033[96m"))

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Parse a .bicep file and list its inputs and outputs.")
    parser.add_argument("file_path", type=str, help="Path to the .bicep file.")

    args = parser.parse_args()

    parse_bicep_file(args.file_path)


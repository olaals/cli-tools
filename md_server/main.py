import argparse
import os
from .server import ThreadedTCPServer
from .custom_handler import CustomHandler
from .file_watcher import FileWatcher
from .custom_handler import send_sse_update, global_state
import pycmarkgfm
from pycmarkgfm import options
import re

def render_markdown_to_html(file_path: str) -> str:
    """
    Convert the content of a Markdown file to HTML using GitHub Flavored Markdown.

    :param file_path: Path to the Markdown file.
    :return: HTML content as a string.
    """
    try:
        with open(file_path, 'r', encoding='utf-8') as file:
            markdown_content = file.read()
        markdown_content = swap_mermaid_block(markdown_content)
        # render unsafe html
        html_content = pycmarkgfm.gfm_to_html(markdown_content, options=options.unsafe | options.hardbreaks)
        html_content = html_content.replace("\n", "[NEWLINE]")
        global_state["last_sent"] = html_content

        print(html_content)
        return html_content
    except Exception as e:
        return f"<p>Error rendering Markdown: {e}</p>"

def swap_mermaid_block(text: str) -> str:
    pattern = r'```mermaid\n(.*?)\n```'
    replacement = r'<pre class="mermaid">\n\1\n</pre>'
    return re.sub(pattern, replacement, text, flags=re.DOTALL)

def file_change_callback(file_path: str):
    print("Callback called")
    if os.path.splitext(file_path)[1].lower() == '.md':
        html_content = render_markdown_to_html(file_path)
        print(html_content)
        print(f"Markdown file {file_path} changed, sending SSE update")
        send_sse_update(html_content)


def run_md_server(directory=None, port=8000):

    if directory is None:
        directory = os.getcwd()


    try:
        # Initialize FileWatcher with the directory to watch and the callback function
        file_watcher = FileWatcher(directory, file_change_callback)
        
        # Start the file watcher in a separate thread
        file_watcher.start_watching()

        # Set up and start the threaded TCP server
        with ThreadedTCPServer(("", port), CustomHandler) as httpd:
            print(f"Serving files from {directory} on http://localhost:{port}")
            httpd.serve_forever()
    except KeyboardInterrupt:
        print("Server is stopping...")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="File Server CLI")
    parser.add_argument("--port", type=int, default=8000, help="Port to run the server on")
    parser.add_argument("--dir", default=os.getcwd(), help="Directory to serve")
    args = parser.parse_args()
    run_md_server()


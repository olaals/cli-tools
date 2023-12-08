import http.server
import os
import urllib
import urllib.parse
from typing import List
from .html_gen import get_html, markdown_update_div
from email.parser import BytesParser
from io import BytesIO
from .graph_html import get_html_graph


global global_state
global_state = {}
global_state["last_sent"] = markdown_update_div().render_html()

subscribers = []

class CustomHandler(http.server.SimpleHTTPRequestHandler):
    """
    Custom HTTP request handler.
    """

    def do_GET(self):
        print("In do_GET")
        print(global_state["last_sent"])
        if self.path.startswith('/static/'):
            self.serve_static_file()
        elif self.path == '/sse':
            self.handle_sse_request()
        elif self.path == '/last-sent':
            self.last_sent()
        elif self.path == '/':
            self.serve_index_html()
        elif self.path.startswith('/change-theme'):
            print("path starts with /change-theme")
            self.change_theme()
        elif self.path == '/get_graph':
            self.get_graph()
        else:
            # Default handler for other paths
            super().do_GET()

    def do_POST(self):
        if self.path == '/upload_image':
            self.handle_image_upload()
        else:
            super().do_POST()

    def get_graph(self):
        self.send_response(200)
        self.send_header("Content-type", "text/html")
        self.end_headers()
        graph_html = get_html_graph().render_html()
        self.wfile.write(graph_html.encode('utf-8'))




    def last_sent(self):
        self.send_response(200)
        self.send_header("Content-type", "text/html")
        self.end_headers()
        self.wfile.write(global_state["last_sent"].encode('utf-8'))

    def get_dir_of_current_file(self):
        """
        Returns the directory of the current file.
        """
        import os
        return os.path.dirname(os.path.realpath(__file__))



    def serve_static_file(self):
        """
        Serves static files from the static directory.
        """
        file_name = os.path.basename(self.path)
        file_path = os.path.join(self.get_dir_of_current_file(), 'static', file_name)

        if os.path.exists(file_path) and os.path.isfile(file_path):
            self.send_response(200)
            self.send_header("Content-type", "text/css" if file_name.endswith('.css') else "text/plain")
            self.end_headers()

            with open(file_path, 'rb') as file:
                self.wfile.write(file.read())
        else:
            self.send_error(404, "File Not Found")

    def handle_image_upload(self):
        # Read the length of the data
        content_length = int(self.headers['Content-Length'])
        post_data = self.rfile.read(content_length)

        # Parse the multipart data using the email parser
        parser = BytesParser()
        msg = parser.parsebytes(post_data)

        # Assuming the file field is named 'file'
        if 'file' in msg:
            file_part = msg.get_payload(0)
            file_data = file_part.get_payload(decode=True)
            file_name = file_part.get_filename()

            # Save the file
            upload_dir = 'uploads'
            os.makedirs(upload_dir, exist_ok=True)
            file_path = os.path.join(upload_dir, file_name)
            with open(file_path, 'wb') as file:
                file.write(file_data)

            # Send a response to the client
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'File uploaded successfully.')
        else:
            # File field not found
            self.send_error(400, "File field not found")

    def change_theme(self):
        print("In change_theme", self.path)
        query_components = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
        print(f"Query components: {query_components}")
        css_file_name = query_components.get('value', ['github-markdown-light.css'])[0]
        print(f"CSS file name: {css_file_name}")
        
        
        self.send_response(200)
        self.send_header("Content-type", "text/html")
        self.end_headers()
        value = f'<link id="theme-link" rel="stylesheet" href="/{css_file_name}">'.encode('utf-8')
        print(f"Value: {value}")

        self.wfile.write(value)

    def handle_sse_request(self):
        """
        Handles server-sent events (SSE) connections.
        """
        # Implement the logic for handling SSE connections here
        # This is a placeholder implementation
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.end_headers()

        global subscribers
        subscribers.append(self.wfile)
        # Add logic to keep the connection open and send data
        #
    def get_css_options(self) -> List[str]:
        """
        Returns a list of CSS options based on the files in static
        """
        current_file_dir = self.get_dir_of_current_file()
        static_dir = os.path.join(current_file_dir, 'static')
        css_files = [file for file in os.listdir(static_dir) if file.endswith('.css')]
        return css_files


    def serve_index_html(self):
        """
        Serves the index HTML page.
        """
        css_options = self.get_css_options()
        html_str = get_html(css_options)



        self.send_response(200)
        self.send_header("Content-type", "text/html")
        self.end_headers()
        self.wfile.write(html_str.encode('utf-8'))


def send_sse_update(data: str):
    """
    Send data to all subscribers.
    """

    #data = data.replace("\n", "[NEWLINE]")
    message = f"data: {data}\n\n"
    print(f"Sending message: {message}")
    #message = "data: Test message\n\n"
    # print subscribers
    print(f"Subscribers: {subscribers}")
    for sub in subscribers[:]:
        print(f"Sending to subscriber: {sub}")
        try:
            sub.write(message.encode('utf-8'))
            sub.flush()
        except:
            subscribers.remove(sub)


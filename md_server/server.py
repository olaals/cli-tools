import socketserver

class ThreadedTCPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    """
    This class extends TCPServer with Threading capabilities. It allows the server
    to handle requests in separate threads. Each client connection will be handled
    in a new thread, enabling simultaneous connections without blocking.
    """
    allow_reuse_address = True  # Allows the server to bind to the same address again

    def __init__(self, server_address, RequestHandlerClass, bind_and_activate=True):
        """
        Initialize the ThreadedTCPServer.

        :param server_address: A tuple (host, port) where the server will listen
        :param RequestHandlerClass: The handler class for managing client requests
        :param bind_and_activate: Automatically binds and activates the server if True
        """
        super().__init__(server_address, RequestHandlerClass, bind_and_activate)

    def server_activate(self):
        """
        Called by the server's constructor to activate the server.
        """
        super().server_activate()
        print(f"Serving on {self.server_address[0]}:{self.server_address[1]}")

    def shutdown(self):
        """
        Shuts down the server and closes the socket.
        """
        super().shutdown()
        print("Server is shutting down...")

    # Additional methods or overrides can be added here if needed


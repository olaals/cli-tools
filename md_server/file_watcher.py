import os
import time
import hashlib
from threading import Thread

class FileWatcher:
    def __init__(self, directory: str, update_callback):
        self.directory = directory
        self.last_modified_file = None
        self.last_file_checksum = None
        self.update_callback = update_callback

    def compute_md5_checksum(self, file_path: str) -> str:
        hasher = hashlib.md5()
        with open(file_path, 'rb') as file:
            buf = file.read()
            hasher.update(buf)
        return hasher.hexdigest()

    def find_last_modified_file(self):
        while True:
            all_files = []
            for root, dirs, files in os.walk(self.directory):
                for file in files:
                    full_path = os.path.join(root, file)
                    if os.path.isfile(full_path):
                        all_files.append(full_path)
            
            if all_files:
                latest_file = max(all_files, key=os.path.getmtime)
                new_checksum = self.compute_md5_checksum(latest_file)
                if latest_file != self.last_modified_file or new_checksum != self.last_file_checksum:
                    self.last_modified_file = latest_file
                    self.last_file_checksum = new_checksum
                    self.update_callback(self.last_modified_file)
                    print(f"New file found: {self.last_modified_file}, sending to callback")
            time.sleep(0.15)  # Adjust the sleep time as needed



    def start_watching(self):
        thread = Thread(target=self.find_last_modified_file)
        thread.daemon = True
        thread.start()


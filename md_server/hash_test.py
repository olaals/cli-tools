import hashlib
import xxhash
import time

def hash_data_md5(data: str) -> str:
    """ Hashes data using MD5 algorithm. """
    hasher = hashlib.md5()
    hasher.update(data.encode('utf-8'))
    return hasher.hexdigest()

def hash_data_xxhash(data: str) -> str:
    """ Hashes data using xxHash algorithm. """
    hasher = xxhash.xxh64(data)
    return hasher.hexdigest()

def time_hash_function(hash_function, data: str) -> float:
    start_time = time.time()
    for _ in range(1000):
        _ = hash_function(data)
    return time.time() - start_time

# Example usage
try:
    data = "example data" * 300

    #time_md5 = time_hash_function(hash_data_md5, data)
    #print(f"MD5 Time taken: {time_md5}")

    time_xxhash = time_hash_function(hash_data_xxhash, data)
    print(f"xxHash Time taken in milliseconds: {time_xxhash * 1000}")

except Exception as e:
    print(f"Error: {e}")


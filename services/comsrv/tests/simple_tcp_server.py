#!/usr/bin/env python3
"""Simple TCP server for testing - simulates basic Modbus responses"""
import socket
import threading
import sys
import time
import struct

def handle_client(client_socket, address):
    """Handle a client connection"""
    print(f"Connection from {address}")
    try:
        while True:
            # Receive data
            data = client_socket.recv(1024)
            if not data:
                break
            
            print(f"Received {len(data)} bytes: {data.hex()}")
            
            # Simple echo response (not real Modbus, but enough for connection test)
            # Modbus TCP header: Transaction ID (2) + Protocol ID (2) + Length (2) + Unit ID (1)
            # Function code (1) + data
            if len(data) >= 8:
                # Echo back with a simple response
                response = data[:6]  # Copy header
                response += b'\x00\x03'  # Length = 3
                response += data[6:7]  # Unit ID
                response += data[7:8]  # Function code
                response += b'\x00'  # Dummy data
                client_socket.send(response)
                print(f"Sent response: {response.hex()}")
            
            time.sleep(0.1)
    except Exception as e:
        print(f"Error handling client {address}: {e}")
    finally:
        client_socket.close()
        print(f"Connection closed from {address}")

def main():
    host = sys.argv[1] if len(sys.argv) > 1 else "0.0.0.0"
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 5502
    
    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    
    try:
        server_socket.bind((host, port))
        server_socket.listen(5)
        print(f"Simple TCP server listening on {host}:{port}")
        
        while True:
            client_socket, address = server_socket.accept()
            client_thread = threading.Thread(target=handle_client, args=(client_socket, address))
            client_thread.start()
    except KeyboardInterrupt:
        print("\nShutting down server...")
    finally:
        server_socket.close()

if __name__ == "__main__":
    main()
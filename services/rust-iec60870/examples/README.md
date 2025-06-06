# rust-iec60870 Examples

This directory contains example applications demonstrating the usage of the rust-iec60870 library for working with IEC 60870-5-101/104 protocols.

## Examples List

1. **iec104_client.rs**: Demonstrates an IEC 60870-5-104 client that connects to a server, sends commands, and receives data.
   
2. **iec104_server.rs**: Shows how to implement an IEC 60870-5-104 server that listens for connections, handles commands, and sends periodic measurements.
   
3. **iec101_client.rs**: Demonstrates an IEC 60870-5-101 client operating in balanced mode, communicating over a serial connection.
   
4. **error_handling.rs**: Shows various approaches to error handling when working with the library.

## Running the Examples

To run an example, use the following command from the project root directory:

```bash
cargo run --example <example_name>
```

For instance, to run the IEC 60870-5-104 client example:

```bash
cargo run --example iec104_client
```

## Configuration

Before running the examples, you may need to modify some parameters to match your environment:

### For IEC 60870-5-104 Examples:
- Update IP address and port in the client example
- Configure the correct bind address in the server example
- Adjust timeout parameters if needed

### For IEC 60870-5-101 Example:
- Set the correct serial port (e.g., `/dev/ttyS0` on Linux, `COM1` on Windows)
- Configure appropriate baud rate and link parameters

## Debugging

These examples use the `env_logger` crate for logging. To enable debug logs, set the `RUST_LOG` environment variable:

```bash
# On Linux/macOS
RUST_LOG=debug cargo run --example iec104_client

# On Windows PowerShell
$env:RUST_LOG="debug"; cargo run --example iec104_client
```

## Example Descriptions

### IEC 60870-5-104 Client (iec104_client.rs)

This example demonstrates how to:
- Configure and establish a connection to an IEC 60870-5-104 server
- Start the data transfer process
- Send a single command
- Receive and process different types of ASDUs

### IEC 60870-5-104 Server (iec104_server.rs)

This example shows how to:
- Configure and start an IEC 60870-5-104 server
- Accept client connections
- Handle incoming commands (single commands, interrogation)
- Send periodic measurement data
- Respond with command confirmations

### IEC 60870-5-101 Client (iec101_client.rs)

This example demonstrates how to:
- Configure a balanced mode IEC 60870-5-101 client
- Connect to a device via serial port
- Send reset process and interrogation commands
- Process various types of data points

### Error Handling (error_handling.rs)

This example showcases various error handling techniques:
- Basic error propagation using the `?` operator
- Custom error handling with pattern matching
- Using Result combinators
- More concise error handling with try blocks

## Next Steps

After exploring these examples, you can:
1. Modify them to match your specific requirements
2. Integrate them into your own applications
3. Explore more advanced features of the library 
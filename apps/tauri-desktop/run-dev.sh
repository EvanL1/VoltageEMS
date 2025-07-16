#!/bin/bash

echo "🚀 Starting VoltageEMS Desktop Application Development Server..."
echo ""
echo "📦 Installing dependencies with bun..."
bun install

echo ""
echo "🔧 Starting Vite development server..."
echo ""
echo "Once the server starts, you can:"
echo "1. Run 'bun run tauri dev' in another terminal to start the Tauri app"
echo "2. Or visit http://localhost:5173 to view in browser"
echo ""
echo "Default login: admin / admin123"
echo ""

# Start the development server
bun run dev
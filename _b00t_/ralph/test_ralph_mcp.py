#!/usr/bin/env python3
"""Quick test of Ralph MCP server capabilities."""

import asyncio
import subprocess
import sys
import time
from fastmcp import Client


async def test_ralph_mcp():
    """Test Ralph MCP tools and resources."""
    # Start Ralph MCP server in background
    try:
        proc = subprocess.Popen(
            [
                "uv",
                "run",
                "python",
                "-m",
                "ralph",
                "--mcp",
                "--transport",
                "http",
                "--host",
                "127.0.0.1",
                "--port",
                "8766",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except Exception as e:
        print(f"❌ Failed to start Ralph MCP server process: {e}")
        sys.exit(1)
    
    # Wait for server to start
    print("⏳ Waiting for Ralph MCP server to start...")
    time.sleep(3)
    
    # Check if process is still running
    if proc.poll() is not None:
        stdout, stderr = proc.communicate()
        print(f"❌ Server process exited unexpectedly with code {proc.returncode}")
        print(f"   stdout: {stdout.decode('utf-8', errors='ignore')}")
        print(f"   stderr: {stderr.decode('utf-8', errors='ignore')}")
        sys.exit(1)
    
    try:
        # Connect to server with error handling
        try:
            async with Client("http://localhost:8766/mcp") as client:
                print("✅ Connected to Ralph MCP server\n")
                
                # Test get_task_status tool
                print("📋 Testing get_task_status tool...")
                try:
                    result = await client.call_tool("get_task_status", {})
                    if not isinstance(result, dict):
                        print(f"   ❌ Unexpected task status response (not a dict): {result!r}\n")
                    elif not all(
                        key in result
                        for key in ("project", "completed", "total_tasks", "completion_percentage")
                    ):
                        print(f"   ❌ Incomplete task status response (missing keys): {result!r}\n")
                    else:
                        print(f"   Project: {result['project']}")
                        print(
                            f"   Completed: {result['completed']}/{result['total_tasks']} "
                            f"({result['completion_percentage']}%)\n"
                        )
                except Exception as e:
                    print(f"   ❌ get_task_status tool call failed: {e}\n")
                
                # Test get_ralph_status tool
                print("📊 Testing get_ralph_status tool...")
                try:
                    result = await client.call_tool("get_ralph_status", {})
                    if not isinstance(result, dict):
                        print(f"   ❌ Unexpected Ralph status response (not a dict): {result!r}\n")
                    elif "status" not in result:
                        print(f"   ❌ Incomplete Ralph status response (missing 'status'): {result!r}\n")
                    else:
                        print(f"   Status: {result['status']}")
                        print(f"   Total lines: {result.get('total_lines', 0)}\n")
                except Exception as e:
                    print(f"   ❌ get_ralph_status tool call failed: {e}\n")
                
                # Test resources
                print("📂 Testing ralph://tasks resource...")
                try:
                    resources = await client.list_resources()
                    tasks_resources = [r for r in resources if "tasks" in r.uri]
                    if tasks_resources:
                        print(f"   Found resource: {tasks_resources[0].uri}\n")
                    else:
                        print("   ⚠️  No tasks resources found\n")
                except Exception as e:
                    print(f"   ❌ list_resources call failed: {e}\n")
                
                print("✅ All tests completed!")
                
        except ConnectionError as e:
            print(f"❌ Failed to connect to Ralph MCP server: {e}")
            print(f"   Make sure the server is running on http://localhost:8766/mcp")
            sys.exit(1)
        except Exception as e:
            print(f"❌ Unexpected error during client connection or testing: {e}")
            sys.exit(1)
            
    finally:
        # Clean up server process
        try:
            proc.terminate()
            proc.wait(timeout=2)
            print("\n🛑 Server stopped")
        except subprocess.TimeoutExpired:
            print("\n⚠️  Server did not stop gracefully, killing...")
            proc.kill()
            proc.wait()
        except Exception as e:
            print(f"\n⚠️  Error stopping server: {e}")


if __name__ == "__main__":
    asyncio.run(test_ralph_mcp())

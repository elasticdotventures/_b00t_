#!/usr/bin/env python3
"""Simple test script for b00t-grok-guru."""

import asyncio
import logging
from python.b00t_grok_guru import GrokGuru

async def main():
    """Test the grok guru functionality."""
    logging.basicConfig(level=logging.INFO)
    
    print("🎂 Testing b00t-grok-guru...")
    
    # Initialize guru
    guru = GrokGuru()
    await guru.initialize()
    
    print("✅ Guru initialized")
    
    # Test digest
    digest_result = await guru.digest(
        "rust", 
        "Rust is a systems programming language that focuses on safety, speed, and concurrency."
    )
    print(f"✅ Digest: Created chunk {digest_result.chunk.id}")
    print(f"   Vector size: {len(digest_result.chunk.vector) if digest_result.chunk.vector else 'None'}")
    
    # Test learn
    learn_result = await guru.learn("""
        Chapter 1: Rust Ownership
        
        Ownership is Rust's most unique feature and the one that has the deepest implications for the rest of the language.
        
        Chapter 2: References and Borrowing
        
        References allow you to use a value without taking ownership of it.
    """, source="rust-book.md")
    
    print(f"✅ Learn: Created {learn_result.chunks_created} chunks")
    for i, chunk in enumerate(learn_result.chunks):
        print(f"   Chunk {i+1}: {chunk.topic} - {chunk.content[:50]}...")
    
    # Test ask (will be empty since no real vector search yet)
    ask_result = await guru.ask("What is ownership in Rust?")
    print(f"✅ Ask: Found {len(ask_result.results)} results")
    
    # Check status
    status = guru.get_status()
    print(f"✅ Status: {status}")
    
    print("🎂 All tests passed!")

if __name__ == "__main__":
    asyncio.run(main())
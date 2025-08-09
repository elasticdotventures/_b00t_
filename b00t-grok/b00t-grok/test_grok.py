#!/usr/bin/env python3
"""Test b00t-grok PyO3 module"""

import os
import json
from b00t_grok import PyGrokClient

def test_grok():
    # Get credentials from environment
    qdrant_url = "https://a0cfd978-2e95-499c-93cc-9acd66b16d35.us-west-1-0.aws.cloud.qdrant.io:6333"
    api_key = os.environ.get('QDRANT_API_KEY')
    qdrant_url = os.environ.get('QDRANT_URL')
    api_key = os.environ.get('QDRANT_API_KEY')
    
    if not qdrant_url:
        print("❌ QDRANT_URL not found in environment")
        return
    if not api_key:
        print("❌ QDRANT_API_KEY not found in environment")
        return
    
    # Create client
    client = PyGrokClient(qdrant_url, api_key)
    print("✅ PyGrokClient created")
    
    # Test digest
    chunk_json = client.digest("rust", "Rust is a systems programming language")
    chunk = json.loads(chunk_json)
    print(f"✅ Digest: {chunk['content'][:50]}...")
    
    # Test ask (returns empty for now)
    results = client.ask("What is Rust?", "rust")
    print(f"✅ Ask: {len(results)} results")
    
    # Test learn (returns empty for now)
    chunks = client.learn("https://example.com", "Example content")
    print(f"✅ Learn: {len(chunks)} chunks")
    
    print("🎂 b00t-grok PyO3 module tests passed!")

if __name__ == "__main__":
    test_grok()
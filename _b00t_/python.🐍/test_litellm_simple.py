#!/usr/bin/env python3
"""
Simple LiteLLM Proxy Test - b00t Style
Tests proxy connectivity without external dependencies
"""

import json
import sys
from pathlib import Path
from urllib.request import urlopen, Request
from urllib.error import URLError, HTTPError

def test_proxy_health(proxy_url: str = "http://localhost:4000") -> bool:
    """Test if LiteLLM proxy is running"""
    try:
        with urlopen(f"{proxy_url}/health", timeout=5) as response:
            if response.status == 200:
                print(f"✅ Proxy healthy at {proxy_url}")
                return True
            else:
                print(f"❌ Proxy unhealthy: HTTP {response.status}")
                return False
    except URLError:
        print(f"❌ Proxy not running at {proxy_url}")
        return False
    except Exception as e:
        print(f"❌ Health check failed: {e}")
        return False

def test_models_endpoint(proxy_url: str = "http://localhost:4000", master_key: str = "sk-b00t-dev-key-please-change-me") -> list:
    """Test models endpoint"""
    try:
        req = Request(
            f"{proxy_url}/v1/models",
            headers={"Authorization": f"Bearer {master_key}"}
        )
        with urlopen(req, timeout=10) as response:
            if response.status == 200:
                data = json.loads(response.read().decode())
                models = [model["id"] for model in data.get("data", [])]
                print(f"✅ Found {len(models)} models: {', '.join(models)}")
                return models
            else:
                print(f"❌ Models endpoint failed: HTTP {response.status}")
                return []
    except Exception as e:
        print(f"❌ Models endpoint error: {e}")
        return []

def test_completion(model: str, proxy_url: str = "http://localhost:4000", master_key: str = "sk-b00t-dev-key-please-change-me") -> str:
    """Test completion with a model"""
    try:
        data = {
            "model": model,
            "messages": [{"role": "user", "content": "Hello! Respond with just: b00t integrated ✅"}],
            "max_tokens": 20,
            "temperature": 0.1
        }
        
        req = Request(
            f"{proxy_url}/chat/completions",
            data=json.dumps(data).encode(),
            headers={
                "Content-Type": "application/json",
                "Authorization": f"Bearer {master_key}"
            }
        )
        
        with urlopen(req, timeout=30) as response:
            if response.status == 200:
                result = json.loads(response.read().decode())
                content = result["choices"][0]["message"]["content"]
                print(f"✅ {model}: {content.strip()}")
                return content.strip()
            else:
                print(f"❌ {model} completion failed: HTTP {response.status}")
                return ""
    except Exception as e:
        print(f"❌ {model} completion error: {e}")
        return ""

def main():
    """Main test runner"""
    print("🥾 b00t LiteLLM Simple Test")
    print("=" * 30)
    
    # Check if we're in the right directory
    if not Path("_b00t_/litellm.just").exists():
        print("❌ Run this script from the dotfiles root directory")
        sys.exit(1)
    
    proxy_url = "http://localhost:4000"
    master_key = "sk-b00t-dev-key-please-change-me"
    
    tests_passed = 0
    total_tests = 0
    
    # Test 1: Health check
    print("📊 Testing proxy health...")
    total_tests += 1
    if test_proxy_health(proxy_url):
        tests_passed += 1
    else:
        print("🚫 Proxy is down, skipping other tests")
        print(f"\n🏁 Summary: {tests_passed}/{total_tests} tests passed")
        sys.exit(1)
    
    # Test 2: Models endpoint
    print("\n📋 Testing models endpoint...")
    total_tests += 1
    models = test_models_endpoint(proxy_url, master_key)
    if models:
        tests_passed += 1
        test_models = models[:2]  # Test first 2 models
    else:
        test_models = ["claude-3-5-sonnet", "llama-3-8b-instruct"]  # Fallback
    
    # Test 3: Completions for available models
    print("\n🤖 Testing completions...")
    for model in test_models:
        print(f"  Testing {model}...")
        total_tests += 1
        if test_completion(model, proxy_url, master_key):
            tests_passed += 1
    
    # Summary
    print(f"\n🏁 Summary: {tests_passed}/{total_tests} tests passed")
    success_rate = (tests_passed / total_tests) * 100 if total_tests > 0 else 0
    print(f"   Success rate: {success_rate:.1f}%")
    
    if tests_passed == total_tests:
        print("🎉 All tests passed! LiteLLM integration working correctly.")
        sys.exit(0)
    else:
        print("⚠️  Some tests failed. Check proxy configuration and API keys.")
        sys.exit(1)

if __name__ == "__main__":
    main()
#!/usr/bin/env python3
"""
LiteLLM Integration Test Script
Tests the LiteLLM Python SDK with b00t-configured proxy server
"""

import os
import sys
from typing import Dict, List, Any, Optional
from dataclasses import dataclass
import json
import requests
from pathlib import Path

# Add returns for b00t-style error handling
try:
    from returns.result import Result, Success, Failure
    from returns.option import Option, Some, Nothing
except ImportError:
    print("⚠️  Installing returns module for b00t-style error handling...")
    os.system("uv tool install returns --python 3.12")
    from returns.result import Result, Success, Failure
    from returns.option import Option, Some, Nothing

# Install litellm if not available
try:
    import litellm
except ImportError:
    print("⚠️  Installing litellm Python SDK...")
    os.system("uv tool install litellm --python 3.12")
    import litellm

@dataclass
class TestConfig:
    """Test configuration for LiteLLM proxy"""
    proxy_url: str = "http://localhost:4000"
    master_key: str = "sk-b00t-dev-key-please-change-me"
    test_models: List[str] = None
    
    def __post_init__(self):
        if self.test_models is None:
            self.test_models = ["claude-3-5-sonnet", "llama-3-8b-instruct"]

class LiteLLMTester:
    """Test LiteLLM integration with b00t proxy"""
    
    def __init__(self, config: TestConfig):
        self.config = config
        self.results: List[Dict[str, Any]] = []
        
        # Configure litellm for proxy
        litellm.api_base = config.proxy_url
        litellm.api_key = config.master_key
    
    def check_proxy_health(self) -> Result[Dict[str, Any], str]:
        """Check if LiteLLM proxy is running and healthy"""
        try:
            response = requests.get(f"{self.config.proxy_url}/health", timeout=5)
            if response.status_code == 200:
                return Success({"status": "healthy", "response": response.json()})
            else:
                return Failure(f"Proxy unhealthy: HTTP {response.status_code}")
        except requests.exceptions.ConnectionError:
            return Failure("Proxy not running - connection refused")
        except Exception as e:
            return Failure(f"Health check failed: {str(e)}")
    
    def list_available_models(self) -> Result[List[str], str]:
        """Get list of available models from proxy"""
        try:
            response = requests.get(
                f"{self.config.proxy_url}/v1/models",
                headers={"Authorization": f"Bearer {self.config.master_key}"},
                timeout=10
            )
            if response.status_code == 200:
                models_data = response.json()
                model_names = [model["id"] for model in models_data.get("data", [])]
                return Success(model_names)
            else:
                return Failure(f"Failed to list models: HTTP {response.status_code}")
        except Exception as e:
            return Failure(f"Model listing failed: {str(e)}")
    
    def test_model_completion(self, model: str, test_prompt: str = "Hello! Respond with just: b00t integrated ✅") -> Result[str, str]:
        """Test completion with a specific model"""
        try:
            # Use litellm SDK to make completion request
            response = litellm.completion(
                model=model,
                messages=[{"role": "user", "content": test_prompt}],
                max_tokens=20,
                temperature=0.1,
                api_base=self.config.proxy_url,
                api_key=self.config.master_key
            )
            
            if response and response.choices:
                content = response.choices[0].message.content
                return Success(content.strip())
            else:
                return Failure("Empty response from model")
                
        except Exception as e:
            return Failure(f"Completion failed for {model}: {str(e)}")
    
    def run_comprehensive_test(self) -> Dict[str, Any]:
        """Run full test suite"""
        print("🧪 Starting LiteLLM Integration Test Suite...")
        test_results = {
            "proxy_health": None,
            "available_models": None,
            "model_tests": {},
            "summary": {"total_tests": 0, "passed": 0, "failed": 0}
        }
        
        # Test 1: Proxy Health Check
        print("📊 Checking proxy health...")
        health_result = self.check_proxy_health()
        match health_result:
            case Success(health_info):
                print(f"✅ Proxy is healthy: {health_info['status']}")
                test_results["proxy_health"] = "healthy"
                test_results["summary"]["passed"] += 1
            case Failure(error):
                print(f"❌ Proxy health check failed: {error}")
                test_results["proxy_health"] = f"failed: {error}"
                test_results["summary"]["failed"] += 1
                # If proxy is down, skip model tests
                test_results["summary"]["total_tests"] = 1
                return test_results
        
        test_results["summary"]["total_tests"] += 1
        
        # Test 2: List Available Models
        print("📋 Listing available models...")
        models_result = self.list_available_models()
        match models_result:
            case Success(models):
                print(f"✅ Found {len(models)} models: {', '.join(models)}")
                test_results["available_models"] = models
                test_results["summary"]["passed"] += 1
            case Failure(error):
                print(f"❌ Failed to list models: {error}")
                test_results["available_models"] = f"failed: {error}"
                test_results["summary"]["failed"] += 1
                # Use configured test models if discovery fails
                models = self.config.test_models
        
        test_results["summary"]["total_tests"] += 1
        
        # Test 3: Model Completion Tests
        print("🤖 Testing model completions...")
        for model in self.config.test_models:
            print(f"  Testing {model}...")
            completion_result = self.test_model_completion(model)
            test_results["summary"]["total_tests"] += 1
            
            match completion_result:
                case Success(response_content):
                    print(f"  ✅ {model}: {response_content}")
                    test_results["model_tests"][model] = {
                        "status": "success",
                        "response": response_content
                    }
                    test_results["summary"]["passed"] += 1
                case Failure(error):
                    print(f"  ❌ {model}: {error}")
                    test_results["model_tests"][model] = {
                        "status": "failed", 
                        "error": error
                    }
                    test_results["summary"]["failed"] += 1
        
        return test_results
    
    def print_summary(self, results: Dict[str, Any]) -> None:
        """Print test summary"""
        summary = results["summary"]
        total = summary["total_tests"]
        passed = summary["passed"]
        failed = summary["failed"]
        
        print("\n🏁 Test Summary:")
        print(f"   Total tests: {total}")
        print(f"   ✅ Passed: {passed}")
        print(f"   ❌ Failed: {failed}")
        print(f"   Success rate: {(passed/total)*100:.1f}%" if total > 0 else "   No tests run")
        
        if failed == 0:
            print("🎉 All tests passed! LiteLLM integration is working correctly.")
        else:
            print("⚠️  Some tests failed. Check proxy configuration and API keys.")


def main():
    """Main test runner"""
    print("🥾 b00t LiteLLM Integration Test")
    print("=" * 40)
    
    # Check if we're in the right directory
    if not Path("_b00t_/litellm.just").exists():
        print("❌ Run this script from the dotfiles root directory")
        sys.exit(1)
    
    # Create test configuration
    config = TestConfig()
    
    # Override with environment variables if available
    config.master_key = os.getenv("LITELLM_MASTER_KEY", config.master_key)
    
    # Run tests
    tester = LiteLLMTester(config)
    results = tester.run_comprehensive_test()
    tester.print_summary(results)
    
    # Save results to file
    results_file = Path("_b00t_/litellm") / "test_results.json"
    results_file.parent.mkdir(exist_ok=True)
    with open(results_file, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\n📁 Test results saved to: {results_file}")
    
    # Exit with appropriate code
    if results["summary"]["failed"] > 0:
        sys.exit(1)
    else:
        sys.exit(0)


if __name__ == "__main__":
    main()
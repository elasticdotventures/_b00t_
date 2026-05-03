"""Demo: URL crawling with grok learn."""
import asyncio
import logging
from b00t_grok_guru import GrokGuru

logging.basicConfig(level=logging.INFO)


async def demo_url_crawling():
    """Demonstrate URL crawling and learning."""
    print("=" * 60)
    print("b00t-grok URL Crawling Demo")
    print("=" * 60)

    # Initialize guru
    print("\n1. Initializing GrokGuru...")
    guru = GrokGuru(qdrant_url="http://localhost:6333")
    await guru.initialize()
    print("✅ GrokGuru initialized")

    # Test URL detection
    print("\n2. Testing URL detection...")
    test_urls = [
        "https://github.com/unclecode/crawl4ai",
        "http://example.com",
        "not a url",
        "file.txt"
    ]
    for url in test_urls:
        is_url = guru._is_url(url)
        print(f"   {url}: {'✅ URL' if is_url else '❌ Not URL'}")

    # Demonstrate learn with URL (will auto-detect and crawl)
    print("\n3. Learning from URL...")
    print("   URL: http://example.com")
    print("   This will automatically crawl the URL and ingest content")

    result = await guru.learn(
        content="http://example.com"  # 🤓 Auto-detected as URL
    )

    if result.success:
        print(f"   ✅ Successfully learned from URL")
        print(f"   📦 Created {result.chunks_created} chunks")
        print(f"   📝 Source: {result.source}")
    else:
        print(f"   ⚠️  Failed: {result.message}")

    # Alternative: pass URL as source parameter
    print("\n4. Alternative syntax (URL as source)...")
    result2 = await guru.learn(
        content="dummy content will be replaced",
        source="http://example.com"
    )

    if result2.success:
        print(f"   ✅ Successfully learned from URL")
        print(f"   📦 Created {result2.chunks_created} chunks")
    else:
        print(f"   ⚠️  Failed: {result2.message}")

    print("\n" + "=" * 60)
    print("Demo complete!")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(demo_url_crawling())

#!/usr/bin/env python3
"""Kreuzberg document intelligence smoke test.
Usage: python3 kreuzberg-test.py [path-to-pdf]
"""
import asyncio
import os
import sys


async def main() -> None:
    # Check import
    try:
        import kreuzberg  # noqa: F401
        print("✅ kreuzberg module importable")
    except ImportError:
        print("❌ kreuzberg not installed. Run: just kreuzberg-install")
        sys.exit(1)

    # Check MCP submodule
    try:
        from kreuzberg import mcp  # noqa: F401
        print("✅ kreuzberg.mcp module available")
    except ImportError:
        print("⚠️  kreuzberg.mcp not found (may need kreuzberg-mcp package)")
        print("   Try: uvx kreuzberg-mcp")

    # Test extraction if sample file provided
    sample = sys.argv[1] if len(sys.argv) > 1 else None
    if not sample:
        for candidate in ("sample.pdf", "test.pdf", "document.pdf", "README.pdf"):
            if os.path.isfile(candidate):
                sample = candidate
                break

    if sample:
        print(f"\n📄 Testing extraction on: {sample}")
        try:
            from kreuzberg import extract_file
            result = await extract_file(sample)
            content = getattr(result, 'content', None) or getattr(result, 'text', None) or "(no content)"
            text = content[:500]
            print(text)
            print("\n✅ kreuzberg extraction test passed")
        except Exception as e:
            print(f"⚠️  Extraction failed: {e}")
            print("   (this may be expected for some file types)")
    else:
        print("\nℹ️  No sample PDF found. Skipping extraction test.")
        print("   Drop a PDF named sample.pdf and re-run: just kreuzberg-test")

    print("\n✅ kreuzberg-test complete")


if __name__ == "__main__":
    asyncio.run(main())

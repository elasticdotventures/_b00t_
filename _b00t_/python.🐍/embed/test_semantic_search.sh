#!/bin/bash
# Test script for embed_anything semantic search integration

set -euo pipefail

echo "🧪 Testing embed_anything integration..."
echo

# Test 1: Check installation
echo "1️⃣  Checking embed_anything installation..."
if python3 -c "import embed_anything" 2>/dev/null; then
    VERSION=$(python3 -c "import embed_anything; print(embed_anything.__version__)")
    echo "   ✅ embed_anything v${VERSION} installed"
else
    echo "   ❌ embed_anything not installed"
    echo "   Run: just embed install"
    exit 1
fi
echo

# Test 2: Load datums
echo "2️⃣  Testing datum loading..."
DATUM_COUNT=$(python3 -c "
import sys
sys.path.insert(0, '/home/brianh/.b00t/_b00t_/python.🐍/embed')
from semantic_datum_search import DatumEmbedder
embedder = DatumEmbedder()
datums = embedder.load_datums()
print(len(datums))
" 2>/dev/null || echo "0")

if [ "$DATUM_COUNT" -gt 0 ]; then
    echo "   ✅ Loaded ${DATUM_COUNT} datums"
else
    echo "   ❌ Failed to load datums"
    exit 1
fi
echo

# Test 3: Generate sample embedding
echo "3️⃣  Testing embedding generation..."
python3 -c "
import sys
sys.path.insert(0, '/home/brianh/.b00t/_b00t_/python.🐍/embed')
from semantic_datum_search import DatumEmbedder

embedder = DatumEmbedder()
datums = embedder.load_datums()[:1]  # Just one datum for speed

print('   Embedding:', datums[0]['name'])
embedded = embedder.embed_datums(datums)

if len(embedded) > 0 and len(embedded[0].embedding) > 0:
    print(f'   ✅ Generated embedding with {len(embedded[0].embedding)} dimensions')
else:
    print('   ❌ Failed to generate embedding')
    sys.exit(1)
" || exit 1
echo

# Test 4: Semantic search
echo "4️⃣  Testing semantic search..."
python3 ~/.b00t/_b00t_/python.🐍/embed/semantic_datum_search.py \
    --query "how to install kubernetes" \
    --top-k 3 \
    2>&1 | head -15
echo

echo "✅ All tests passed!"
echo
echo "Usage examples:"
echo "  just embed search 'how to install rust' 5"
echo "  just embed embed-all"
echo "  just embed test"

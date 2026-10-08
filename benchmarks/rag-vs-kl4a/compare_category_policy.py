"""Baseline-RAG vs KL4A/sopkb comparison, case 3: content designed to confuse a RAG pipeline.

Same method as compare.py, run against examples/category-returns-policy instead: 12
near-duplicate sections (same sentence template, different category name and numbers) -
a known real weakness of small embedding models, where the similarity margin between the
right category and its nearest neighbor can be thin.

Run from the repo root:
    .venv/Scripts/python.exe benchmarks/rag-vs-kl4a/compare_category_policy.py
"""

import pathlib
import re

import chromadb

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = REPO_ROOT / "examples" / "category-returns-policy" / "sources" / "category-policy.md"
CHROMA_DIR = pathlib.Path(__file__).resolve().parent / ".chroma"

QUERY = "What is the restocking fee percentage for Major Appliances returned without original packaging?"


def chunk_by_section(text: str) -> list[dict]:
    """Split on '## ' headings - one chunk per section, title kept with body."""
    parts = re.split(r"(?m)^## ", text)
    chunks = []
    # parts[0] is the '# Product Category Returns & Warranty Policy' title block, skip as its own chunk
    for part in parts[1:]:
        lines = part.strip().splitlines()
        title = lines[0].strip()
        body = "\n".join(lines[1:]).strip()
        chunks.append({"title": title, "text": f"{title}\n\n{body}"})
    return chunks


def main() -> None:
    text = SOURCE.read_text(encoding="utf-8")
    chunks = chunk_by_section(text)

    print(f"Source: {SOURCE.relative_to(REPO_ROOT)}")
    print(f"Chunked into {len(chunks)} section(s):")
    for c in chunks:
        print(f"  - {c['title']!r} ({len(c['text'])} chars)")
    print()

    client = chromadb.PersistentClient(path=str(CHROMA_DIR))
    try:
        client.delete_collection("category-policy")
    except Exception:
        pass
    collection = client.create_collection("category-policy")

    collection.add(
        ids=[f"chunk-{i}" for i in range(len(chunks))],
        documents=[c["text"] for c in chunks],
        metadatas=[{"title": c["title"]} for c in chunks],
    )

    print(f"=== Query: {QUERY!r} ===")
    result = collection.query(query_texts=[QUERY], n_results=4)
    ids = result["ids"][0]
    docs = result["documents"][0]
    dists = result["distances"][0]
    for rank, (cid, doc, dist) in enumerate(zip(ids, docs, dists), start=1):
        similarity = 1 - dist  # Chroma default space is squared-L2 on normalized embeddings
        print(f"  #{rank} {cid}  distance={dist:.4f}  similarity~={similarity:.4f}")
        print(f"      {doc!r}")
    print()


if __name__ == "__main__":
    main()

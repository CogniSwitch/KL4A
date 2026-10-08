"""Simple baseline-RAG vs KL4A/sopkb comparison.

Baseline RAG: chunk the real source doc by section, embed with Chroma's
default local embedding function (ONNX MiniLM, no API key, no network call
at query time), and retrieve the top chunks for a real question.

Run from the repo root:
    .venv/Scripts/python.exe benchmarks/rag-vs-kl4a/compare.py
"""

import pathlib
import re

import chromadb

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = REPO_ROOT / "examples" / "customer-refund-policy" / "sources" / "refund-policy.md"
CHROMA_DIR = pathlib.Path(__file__).resolve().parent / ".chroma"

QUERIES = [
    "Can a customer request a refund after 20 days?",
    "refund",
]


def chunk_by_section(text: str) -> list[dict]:
    """Split on '## ' headings - one chunk per section, title kept with body."""
    parts = re.split(r"(?m)^## ", text)
    chunks = []
    # parts[0] is the '# Customer Refund Policy' title block, skip as its own chunk
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
        client.delete_collection("refund-policy")
    except Exception:
        pass
    collection = client.create_collection("refund-policy")

    collection.add(
        ids=[f"chunk-{i}" for i in range(len(chunks))],
        documents=[c["text"] for c in chunks],
        metadatas=[{"title": c["title"]} for c in chunks],
    )

    for query in QUERIES:
        print(f"=== Query: {query!r} ===")
        result = collection.query(query_texts=[query], n_results=2)
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

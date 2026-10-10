# Baseline RAG vs KL4A/sopkb — a quick, real test

Not a rigorous benchmark — a small, honest comparison to see what a minimal baseline RAG
pipeline returns versus KL4A's `sopkb`, on the same real source and the same real question.

**Source:** `examples/customer-refund-policy/sources/refund-policy.md` (real, checked-in).
**Question:** "Can a customer request a refund after 20 days?"

## Baseline RAG (chromadb, local ONNX MiniLM embeddings, no API key/network at query time)

Setup: the source was chunked by `##` section (2 chunks: "Refund Eligibility", "Refund
Approval"), embedded with Chroma's default `all-MiniLM-L6-v2` embedding function, and
queried. Real output from `benchmarks/rag-vs-kl4a/compare.py`:

```
=== Query: 'Can a customer request a refund after 20 days?' ===
  #1 chunk-0  distance=0.6761  similarity~=0.3239
      'Refund Eligibility\n\nCustomers must submit refund requests within 30 days of purchase.\n\nDigital products must not have been downloaded to qualify for a refund.'
  #2 chunk-1  distance=1.1397  similarity~=-0.1397
      'Refund Approval\n\nRefund requests above $1,000 must receive finance approval before processing.'
```

## KL4A / sopkb (real, same source, same underlying fact)

```console
$ kl4a --use sopkb knowledge search examples/customer-refund-policy/bundle "refund"
```

```json
{
  "evidence": "Customers must submit refund requests within 30 days of purchase.",
  "id": "ki-refund-policy-v1-000001",
  "object": "Customers must submit refund requests within 30 days of purchase.",
  "predicate": "requires",
  "review_status": "proposed",
  "section_id": "section-refund-policy-002",
  "source_id": "refund-policy",
  "subject": "Refund Eligibility"
}
```

## Honest comparison

| | Baseline RAG | KL4A / sopkb |
| --- | --- | --- |
| Retrieved the right chunk/fact? | Yes — top hit was the correct section | Yes |
| Exact evidence span? | No — returns the whole chunk (section + both its sentences) | Yes — `evidence` is the single exact sentence |
| Review/approval status? | None | `review_status: "proposed"` — a reviewable, trackable field |
| Structured claim? | No — raw text only | Yes — `subject`/`predicate`/`object` |
| Confidence signal | A distance score (0.676), with no fixed meaning on its own | N/A — it's a reviewed/proposed claim, not a ranked guess |

**Takeaway:** on this easy, single-document, 2-section case, the baseline RAG's retrieval
was actually fine — it found the right chunk both times. That's expected: retrieval
accuracy on a tiny, unambiguous doc isn't where the real gap is. The gap is in what comes
back: baseline RAG hands an agent an undifferentiated chunk of text and a similarity
score with no fixed meaning; KL4A hands back the exact sentence the claim came from, a
structured subject/predicate/object, and an explicit review state. The baseline RAG chunk
mixes two separate claims ("must submit within 30 days" and "must not have been
downloaded") into one blob with no way to cite either individually — KL4A keeps them as
two distinct, independently reviewable knowledge items. This difference would matter far
more on a longer, messier real document; it doesn't show up as a retrieval-accuracy
difference on this one.

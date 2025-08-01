#!/usr/bin/env python3
"""
Minimal test using sentence-transformers which handles dependencies better
"""

try:
    from sentence_transformers import CrossEncoder
    print("✓ sentence-transformers imported successfully")
except ImportError:
    print("Installing sentence-transformers...")
    import subprocess
    subprocess.check_call([sys.executable, "-m", "pip", "install", "sentence-transformers"])
    from sentence_transformers import CrossEncoder

# Test inputs
queries = [
    "how does authentication work",
    "foobar random nonsense gibberish"
]

document = """Authentication is the process of verifying the identity of a user, device, or system. 
In web applications, authentication typically involves checking credentials like usernames 
and passwords against a database."""

# Load model
print("Loading cross-encoder model...")
model = CrossEncoder('cross-encoder/ms-marco-TinyBERT-L-2-v2', max_length=512)
print("Model loaded!")

# Score pairs
print("\nScoring query-document pairs:")
print("-" * 50)

scores = []
for query in queries:
    score = model.predict([(query, document)])[0]
    scores.append(score)
    print(f"Query: '{query}'")
    print(f"Score: {score:.6f}\n")

# Compare
print("Comparison:")
print(f"Relevant query score: {scores[0]:.6f}")
print(f"Nonsense query score: {scores[1]:.6f}")
print(f"Difference: {scores[0] - scores[1]:.6f}")

if scores[0] > scores[1] + 0.1:
    print("\n✓ Good: Relevant query scores higher")
else:
    print("\n⚠ Poor discrimination between queries")
from sentence_transformers import SentenceTransformer
from sklearn.metrics.pairwise import cosine_similarity

#model = SentenceTransformer('bert-base-nli-mean-tokens')
model = SentenceTransformer('bert-base-multilingual-cased')

def compute_similarity(s, cs):
    embeds = model.encode([s, *cs])
    return cosine_similarity(embeds[:1], embeds[1:])[0]

if __name__ == '__main__':
    import sqlite3
    db = sqlite3.connect("file:blume.db?mode=ro", uri=True)
    cur = db.cursor().execute("""
SELECT lines.line, google.translation, current.translation
FROM lines
LEFT JOIN translations AS google
ON google.session = 'google' AND google.scriptid = lines.scriptid AND google.address = lines.address
INNER JOIN translations AS current
ON current.session = 'llm-20240201' AND current.scriptid = lines.scriptid AND current.address = lines.address
""")
    for line, gtl, ltl in cur:
        [gs] = compute_similarity(line, [gtl])
        print(gs)

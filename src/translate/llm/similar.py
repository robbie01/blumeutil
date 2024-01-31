from sentence_transformers import SentenceTransformer
from sklearn.metrics.pairwise import cosine_similarity

def compute_similarity(s, cs):
    model = SentenceTransformer('bert-base-nli-mean-tokens')

    # Get BERT embeddings for each sentence
    embeds = model.encode([s, *cs])

    # Calculate cosine similarity between the embeddings
    scores = cosine_similarity(embeds[:1], embeds[1:])[0]

    return scores

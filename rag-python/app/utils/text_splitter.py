def split_text_by_words(text: str, chunk_size: int, chunk_overlap: int) -> list[str]:
    """
    Splits text into chunks of `chunk_size` words with `chunk_overlap` word overlap.
    """
    words = text.split()
    if not words:
        return []

    chunks = []
    i = 0
    step = max(chunk_size - chunk_overlap, 1)
    while i < len(words):
        chunk_words = words[i:i + chunk_size]
        chunk_text = " ".join(chunk_words)
        chunks.append(chunk_text)

        if i + chunk_size >= len(words):
            break

        # When overlap is too large, fall back to a one-word sliding window.
        i += step

    return chunks

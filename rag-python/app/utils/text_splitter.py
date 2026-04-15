def split_text_by_words(text: str, chunk_size: int, chunk_overlap: int) -> list[str]:
    """
    Splits text into chunks of `chunk_size` words with `chunk_overlap` word overlap.
    """
    words = text.split()
    if not words:
        return []

    chunks = []
    i = 0
    while i < len(words):
        chunk_words = words[i:i + chunk_size]
        chunk_text = " ".join(chunk_words)
        chunks.append(chunk_text)
        
        if i + chunk_size >= len(words):
            break
            
        # Move forward by chunk_size - overlap
        i += (chunk_size - chunk_overlap)
        
        # Ensure we always move forward
        step = chunk_size - chunk_overlap
        if step <= 0:
            step = 1 # Fallback to prevent infinite loop
            i += step

    return chunks

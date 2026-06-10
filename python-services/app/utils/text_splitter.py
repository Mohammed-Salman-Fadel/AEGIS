def recursive_token_splitter(text: str, tokenizer, chunk_size: int = 510, chunk_overlap: int = 50) -> list[str]:
    """
    Recursively splits text using semantic boundaries and measures length using the provided tokenizer.
    Ensures chunks do not exceed the chunk_size token limit.
    """
    if not text.strip():
        return []
    
    tokens = tokenizer.encode(text, add_special_tokens=False)
    if len(tokens) <= chunk_size:
        return [text]

    # Try splitting by decreasing levels of semantic importance
    separators = ["\n\n", "\n", ". ", " "]
    
    for separator in separators:
        splits = text.split(separator)
        if len(splits) > 1:
            break
    else:
        # If we couldn't split by any separator (e.g. one massive word), fall back to character splitting
        splits = [text[i:i+chunk_size] for i in range(0, len(text), chunk_size)]
        return splits

    chunks = []
    current_chunk = []
    current_length = 0

    for i, split in enumerate(splits):
        # Re-attach the separator to perfectly reconstruct the original text
        split_text = split + separator if i < len(splits) - 1 else split
        
        # Recursively split if the sub-split itself is too large
        split_tokens = tokenizer.encode(split_text, add_special_tokens=False)
        if len(split_tokens) > chunk_size:
            if current_chunk:
                chunks.append("".join(current_chunk).strip())
                current_chunk = []
                current_length = 0
            
            recursive_chunks = recursive_token_splitter(split_text, tokenizer, chunk_size, chunk_overlap)
            chunks.extend(recursive_chunks)
            continue

        if current_length + len(split_tokens) > chunk_size:
            chunks.append("".join(current_chunk).strip())
            
            overlap_chunk = []
            overlap_length = 0
            for prev_split in reversed(current_chunk):
                prev_tokens = len(tokenizer.encode(prev_split, add_special_tokens=False))
                if overlap_length + prev_tokens <= chunk_overlap:
                    overlap_chunk.insert(0, prev_split)
                    overlap_length += prev_tokens
                else:
                    break
                    
            current_chunk = overlap_chunk
            current_length = overlap_length

        current_chunk.append(split_text)
        current_length += len(split_tokens)

    if current_chunk:
        chunks.append("".join(current_chunk).strip())

    return chunks

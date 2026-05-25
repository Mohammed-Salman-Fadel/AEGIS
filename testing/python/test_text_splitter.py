import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
RAG_ROOT = ROOT / "rag-python"
if str(RAG_ROOT) not in sys.path:
    sys.path.insert(0, str(RAG_ROOT))

from app.utils.text_splitter import split_text_by_words


class TextSplitterTests(unittest.TestCase):
    def test_returns_empty_list_for_blank_input(self) -> None:
        self.assertEqual(split_text_by_words("", chunk_size=5, chunk_overlap=2), [])

    def test_preserves_overlap_without_skipping_words(self) -> None:
        text = "one two three four five six seven"

        chunks = split_text_by_words(text, chunk_size=3, chunk_overlap=1)

        self.assertEqual(
            chunks,
            [
                "one two three",
                "three four five",
                "five six seven",
            ],
        )

    def test_overlap_larger_than_chunk_size_falls_back_to_sliding_window(self) -> None:
        text = "one two three four five"

        chunks = split_text_by_words(text, chunk_size=3, chunk_overlap=5)

        self.assertEqual(
            chunks,
            [
                "one two three",
                "two three four",
                "three four five",
            ],
        )


if __name__ == "__main__":
    unittest.main()

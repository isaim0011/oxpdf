#!/usr/bin/env bash
set -euo pipefail

# scripts/fetch_corpus.sh
# Fetches test PDFs from pdf.js and veraPDF test corpuses into corpus/

CORPUS_DIR="corpus"
mkdir -p "$CORPUS_DIR/pdfjs"
mkdir -p "$CORPUS_DIR/verapdf"

echo "=== Fetching pdf.js test PDFs ==="
# Clone pdf.js with depth 1 and sparse checkout test files if git is available
if [ ! -d "$CORPUS_DIR/pdfjs/.git" ]; then
    git clone --depth 1 --filter=blob:none --sparse https://github.com/mozilla/pdf.js.git "$CORPUS_DIR/pdfjs_repo" || true
    if [ -d "$CORPUS_DIR/pdfjs_repo" ]; then
        cd "$CORPUS_DIR/pdfjs_repo"
        git sparse-checkout set test/pdfs
        cd ../..
        cp -r "$CORPUS_DIR/pdfjs_repo/test/pdfs/"* "$CORPUS_DIR/pdfjs/" || true
        rm -rf "$CORPUS_DIR/pdfjs_repo"
    fi
fi

# Fallback: curl a curated set of standard test PDFs from pdf.js raw repo if sparse checkout failed or empty
PDFJS_FILES=(
    "annotation-line.pdf"
    "annotation-link.pdf"
    "annotation-highlight.pdf"
    "fips140_2.pdf"
    "issue5005.pdf"
    "trace.pdf"
    "calgray.pdf"
    "basicapi.pdf"
)

for file in "${PDFJS_FILES[@]}"; do
    if [ ! -f "$CORPUS_DIR/pdfjs/$file" ]; then
        echo "Downloading pdf.js $file..."
        curl -fsSL "https://raw.githubusercontent.com/mozilla/pdf.js/master/test/pdfs/$file" -o "$CORPUS_DIR/pdfjs/$file" || true
    fi
done

echo "=== Fetching veraPDF test corpus ==="
# Fetch selected veraPDF corpus files (veraPDF-corpus repository)
VERAPDF_FILES=(
    "veraPDF-test-corpus-6-2-3-3-t01-pass-a.pdf"
    "veraPDF-test-corpus-6-2-3-3-t01-fail-a.pdf"
    "veraPDF-test-corpus-6-1-2-t01-pass-a.pdf"
    "veraPDF-test-corpus-6-1-2-t02-pass-a.pdf"
    "veraPDF-test-corpus-6-1-3-t01-pass-a.pdf"
    "veraPDF-test-corpus-6-1-4-t01-pass-a.pdf"
)

for file in "${VERAPDF_FILES[@]}"; do
    if [ ! -f "$CORPUS_DIR/verapdf/$file" ]; then
        echo "Downloading veraPDF $file..."
        curl -fsSL "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/master/PDF_A-1b/6-2%20Graphics/6-2-3%20Colour%20spaces/6-2-3-3%20Device%20colour%20spaces/$file" -o "$CORPUS_DIR/verapdf/$file" || \
        curl -fsSL "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/master/PDF_A-1b/6-1%20File%20structure/$file" -o "$CORPUS_DIR/verapdf/$file" || true
    fi
done

echo "Corpus download complete. Files located in $CORPUS_DIR/"

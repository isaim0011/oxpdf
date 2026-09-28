# scripts/fetch_corpus.ps1
# Powershell script to fetch test PDFs into corpus/

$CorpusDir = "corpus"
$PdfJsDir = "$CorpusDir/pdfjs"
$VerapdfDir = "$CorpusDir/verapdf"

New-Item -ItemType Directory -Force -Path $PdfJsDir | Out-Null
New-Item -ItemType Directory -Force -Path $VerapdfDir | Out-Null

Write-Host "=== Fetching pdf.js test PDFs ==="

$PdfJsFiles = @(
    "annotation-line.pdf",
    "annotation-link.pdf",
    "annotation-highlight.pdf",
    "fips140_2.pdf",
    "issue5005.pdf",
    "trace.pdf",
    "calgray.pdf",
    "basicapi.pdf",
    "issue11847.pdf",
    "issue4437.pdf"
)

foreach ($f in $PdfJsFiles) {
    $dest = "$PdfJsDir/$f"
    if (-not (Test-Path $dest)) {
        Write-Host "Downloading pdf.js $f..."
        $url = "https://raw.githubusercontent.com/mozilla/pdf.js/master/test/pdfs/$f"
        try {
            Invoke-WebRequest -Uri $url -OutFile $dest -TimeoutSec 15
        } catch {
            Write-Warning "Could not fetch $f : $_"
        }
    }
}

Write-Host "=== Fetching veraPDF test PDFs ==="
$VerapdfUrls = @(
    "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/master/PDF_A-1b/6-2%20Graphics/6-2-3%20Colour%20spaces/6-2-3-3%20Device%20colour%20spaces/veraPDF-test-corpus-6-2-3-3-t01-pass-a.pdf",
    "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/master/PDF_A-1b/6-2%20Graphics/6-2-3%20Colour%20spaces/6-2-3-3%20Device%20colour%20spaces/veraPDF-test-corpus-6-2-3-3-t01-fail-a.pdf",
    "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/master/PDF_A-1b/6-1%20File%20structure/6-1-2%20File%20header/veraPDF-test-corpus-6-1-2-t01-pass-a.pdf",
    "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/master/PDF_A-1b/6-1%20File%20structure/6-1-2%20File%20header/veraPDF-test-corpus-6-1-2-t02-pass-a.pdf",
    "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/master/PDF_A-1b/6-1%20File%20structure/6-1-3%20File%20trailer/veraPDF-test-corpus-6-1-3-t01-pass-a.pdf",
    "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/master/PDF_A-1b/6-1%20File%20structure/6-1-4%20Cross%20reference%20table/veraPDF-test-corpus-6-1-4-t01-pass-a.pdf"
)

foreach ($url in $VerapdfUrls) {
    $fname = [System.IO.Path]::GetFileName($url)
    $dest = "$VerapdfDir/$fname"
    if (-not (Test-Path $dest)) {
        Write-Host "Downloading veraPDF $fname..."
        try {
            Invoke-WebRequest -Uri $url -OutFile $dest -TimeoutSec 15
        } catch {
            Write-Warning "Could not fetch $fname : $_"
        }
    }
}

Write-Host "Corpus download complete."

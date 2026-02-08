# hwpers

[![Crates.io](https://img.shields.io/crates/v/hwpers.svg)](https://crates.io/crates/hwpers)
[![Documentation](https://docs.rs/hwpers/badge.svg)](https://docs.rs/hwpers)
[![CI](https://github.com/Indosaram/hwpers/workflows/CI/badge.svg)](https://github.com/Indosaram/hwpers/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

A Rust library for parsing Korean Hangul Word Processor (HWP) files with full layout rendering support.

## Features

### Parser (Reading HWP files)
- **Complete HWP 5.0 Format Support**: Parse all document components including text, formatting, tables, and embedded objects
- **Visual Layout Rendering**: Reconstruct documents with pixel-perfect accuracy when layout data is available
- **Font and Style Preservation**: Extract and apply original fonts, sizes, colors, and text formatting
- **Advanced Layout Engine**: Support for multi-column layouts, line-by-line positioning, and character-level formatting
- **SVG Export**: Render documents to scalable vector graphics
- **Zero-copy Parsing**: Efficient parsing with minimal memory allocation
- **Safe Rust**: Memory-safe implementation with comprehensive error handling

### Writer (Creating HWP files) - v0.3.0+
- **Document Creation**: Full HWP document writing support
- **Rich Text Formatting**: Bold, italic, colors, fonts, sizes
- **Tables**: Creation, styling, cell merging
- **Lists**: Bullets, numbering, Korean/alphabetic/roman formats
- **Images**: PNG/JPEG/BMP/GIF with captions
- **Text Boxes**: Positioned and styled text boxes
- **Hyperlinks**: URL, email, file, and bookmark links
- **Headers/Footers**: Page numbers and custom content
- **Page Layout**: Sizes, margins, orientation, columns, backgrounds

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
hwpers = "0.3"
```

### Basic Usage

```rust
use hwpers::HwpReader;

// Parse an HWP file
let document = HwpReader::from_file("document.hwp")?;

// Extract text content
let text = document.extract_text();
println!("{}", text);

// Access document properties
if let Some(props) = document.get_properties() {
    println!("Pages: {}", props.total_page_count);
}

// Iterate through sections and paragraphs
for (i, section) in document.sections().enumerate() {
    println!("Section {}: {} paragraphs", i, section.paragraphs.len());
    
    for paragraph in &section.paragraphs {
        if let Some(text) = &paragraph.text {
            println!("  {}", text.content);
        }
    }
}
```

### Visual Layout Rendering

```rust
use hwpers::{HwpReader, render::{HwpRenderer, RenderOptions}};

let document = HwpReader::from_file("document.hwp")?;

// Create renderer with custom options
let options = RenderOptions {
    dpi: 96,
    scale: 1.0,
    show_margins: false,
    show_baselines: false,
};

let renderer = HwpRenderer::new(&document, options);
let result = renderer.render();

// Export first page to SVG
if let Some(svg) = result.to_svg(0) {
    std::fs::write("page1.svg", svg)?;
}

println!("Rendered {} pages", result.pages.len());
```

### Creating Documents (v0.3.0+)

```rust
use hwpers::writer::HwpWriter;
use hwpers::model::hyperlink::Hyperlink;

// Create a new document
let mut writer = HwpWriter::new();

// Add formatted text
writer.add_aligned_paragraph(
    "제목",
    hwpers::writer::style::ParagraphAlignment::Center
)?;

// Add hyperlinks
let link = Hyperlink::new_url("Rust", "https://rust-lang.org");
writer.add_paragraph_with_hyperlinks(
    "Visit Rust website",
    vec![link]
)?;

// Configure page layout
writer.set_custom_page_size(210.0, 297.0, // A4 size
    hwpers::model::page_layout::PageOrientation::Portrait)?;
writer.set_page_margins_mm(20.0, 20.0, 20.0, 20.0);

// Add header and footer
writer.add_header("Document Header");
writer.add_footer_with_page_number("Page ", 
    hwpers::model::header_footer::PageNumberFormat::Numeric);

// Save the document
writer.save_to_file("output.hwp")?;
```

### Advanced Formatting Access

```rust
// Access character and paragraph formatting
for section in document.sections() {
    for paragraph in &section.paragraphs {
        // Get paragraph formatting
        if let Some(para_shape) = document.get_para_shape(paragraph.para_shape_id as usize) {
            println!("Indent: {}, Alignment: {}", 
                para_shape.indent, 
                para_shape.get_alignment()
            );
        }
        
        // Get character formatting runs
        if let Some(char_shapes) = &paragraph.char_shapes {
            for pos_shape in &char_shapes.char_positions {
                if let Some(char_shape) = document.get_char_shape(pos_shape.char_shape_id as usize) {
                    println!("Position {}: Size {}, Bold: {}", 
                        pos_shape.position,
                        char_shape.base_size / 100,
                        char_shape.is_bold()
                    );
                }
            }
        }
    }
}
```

## Supported Features

### Document Structure
- ✅ File header and version detection
- ✅ Document properties and metadata
- ✅ Section definitions and page layout
- ✅ Paragraph and character formatting
- ✅ Font definitions (FaceName)
- ✅ Styles and templates

### Content Types
- ✅ Text content with full Unicode support
- ✅ Tables and structured data
- ✅ Control objects (images, OLE objects)
- ✅ Numbering and bullet lists
- ✅ Tab stops and alignment

### Layout and Rendering
- ✅ Page dimensions and margins
- ✅ Multi-column layouts
- ✅ Line-by-line positioning (when available)
- ✅ Character-level positioning (when available)
- ✅ Borders and fill patterns
- ✅ SVG export with accurate positioning

### Advanced Features
- ✅ Compressed document support
- ✅ CFB (Compound File Binary) format handling
- ✅ Multiple encoding support (UTF-16LE)
- ✅ Error recovery and partial parsing

## Command Line Tool

The library includes a command-line tool for inspecting HWP files:

```bash
# Install the tool
cargo install hwpers

# Inspect an HWP file
hwp_info document.hwp
```

## jsontohwpx CLI

JSON API 응답을 HWPX(한글 문서) 파일로 변환하는 CLI 도구입니다.

### 빌드

```bash
cargo build --release
```

빌드 결과물: `target/release/jsontohwpx`

### 테스트

```bash
# 전체 테스트 실행
cargo test

# 테이블 관련 테스트만 실행
cargo test --test jsontohwpx_table_test

# CLI 테스트만 실행
cargo test --test jsontohwpx_cli_test

# Clippy 린트 검사
cargo clippy -- -D warnings
```

### 사용법

```bash
jsontohwpx [OPTIONS] <INPUT>
```

### 인자

| 인자 | 설명 |
|------|------|
| `<INPUT>` | 입력 JSON 파일 경로. `-`를 지정하면 stdin에서 읽습니다. |

### 옵션

| 옵션 | 단축 | 기본값 | 설명 |
|------|------|--------|------|
| `--output <PATH>` | `-o` | `{atclId}.hwpx` | 출력 HWPX 파일 경로 |
| `--base-path <PATH>` | `-b` | `.` | 이미지 기본 경로 (상대 경로 이미지 해석용) |
| `--include-header` | | `false` | 헤더(작성자, 부서, 일시) 포함 강제 |
| `--validate` | | `false` | 검증만 수행 (파일 변환 없음) |
| `--json` | | `false` | 에러를 JSON 형식으로 출력 |
| `--help` | `-h` | | 도움말 출력 |

### 실행 예시

```bash
# 기본 변환 (출력: {atclId}.hwpx)
jsontohwpx input.json

# 출력 경로 지정
jsontohwpx input.json -o output.hwpx

# stdin에서 읽기
cat input.json | jsontohwpx -

# 이미지 기본 경로 지정
jsontohwpx input.json -b ./images -o output.hwpx

# 헤더 포함하여 변환
jsontohwpx input.json --include-header -o output.hwpx

# JSON만 검증 (변환 없음)
jsontohwpx input.json --validate

# 에러를 JSON으로 출력 (CI 연동 시 유용)
jsontohwpx input.json --json -o output.hwpx
```

### 입력 JSON 형식

```json
{
  "schema_version": "1.1",
  "article_id": "DOC001",
  "title": "문서 제목",
  "metadata": {
    "author": "작성자",
    "created_at": "2025-01-30T10:00:00+09:00",
    "updated_at": "2025-01-30T10:00:00+09:00",
    "department": "부서명",
    "board_id": "BBNC100171030",
    "board_name": "공지사항",
    "board_path": ["BGF리테일게시판", "전사공지사항", "공지사항"],
    "board_depth": 3,
    "folder_id": "BFCC100171030",
    "expiry": "영구",
    "views": 0,
    "likes": 0,
    "comments": 0
  },
  "attachments": [],
  "attachment_count": 0,
  "total_attachment_size": 0,
  "contents": [
    { "type": "text", "value": "본문 텍스트" },
    { "type": "table", "value": "<table><tr><td>셀</td></tr></table>" },
    { "type": "image", "url": "image.png" }
  ],
  "content_html": "<p>본문 텍스트</p>"
}
```

#### 콘텐츠 타입

| type | 필드 | 설명 |
|------|------|------|
| `text` | `value` | 텍스트 문자열, 줄바꿈(`\n`) 지원 |
| `table` | `value` | HTML 테이블 (`<table>` 태그, colspan/rowspan 지원) |
| `image` | `url` | 파일 경로 또는 HTTP URL (PNG/JPEG/GIF/WebP/AVIF 지원) |
| `image` | `base64` + `format` | Base64 인코딩 이미지 데이터 |

### 종료 코드

| 코드 | 의미 |
|------|------|
| 0 | 성공 |
| 1 | 입력 오류 (파일 없음, JSON 파싱 실패) |
| 2 | 변환 오류 (빈 테이블, 잘못된 데이터) |
| 3 | I/O 오류 (파일 쓰기 실패) |

### 진행 로그

변환 과정은 stderr로 진행 상황을 출력합니다:

```
[1/3] JSON 파싱 중...
[2/3] 변환 중... (3개 콘텐츠)
[3/3] 파일 저장 중... output.hwpx
변환 완료: output.hwpx
```

## Docker로 실행하기

### 빌드 및 실행

```bash
# 의존성 벤더링 (최초 1회)
cargo vendor

# Docker Compose로 실행
docker compose up -d

# 로그 확인
docker compose logs -f

# 종료
docker compose down
```

### 환경 변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `HOST` | `0.0.0.0` | 바인딩 호스트 |
| `PORT` | `8080` | 서버 포트 |
| `RUST_LOG` | `info` | 로그 레벨 |
| `MAX_REQUEST_SIZE` | `52428800` | 최대 요청 크기 (50MB) |
| `WORKER_COUNT` | `4` | 비동기 워커 수 |
| `FILE_EXPIRY_HOURS` | `24` | 생성 파일 만료 시간 |

### docker-compose.yml 설정

- **Healthcheck**: `/api/v1/health` 엔드포인트로 10초 간격 상태 확인
- **Resource limits**: 메모리 1G, CPU 2코어 제한
- **tmpfs**: `/tmp/jsontohwpx`에 512MB tmpfs 마운트 (변환 파일 임시 저장)

## REST API

서버 시작 후 Swagger UI에서 전체 API 문서를 확인할 수 있습니다: `http://localhost:8080/swagger-ui/`

### 엔드포인트 목록

| 메서드 | 경로 | 설명 |
|--------|------|------|
| `POST` | `/api/v1/convert` | 동기 변환 (즉시 HWPX 반환) |
| `POST` | `/api/v1/convert/async` | 비동기 변환 (작업 ID 반환) |
| `GET` | `/api/v1/jobs/:id` | 비동기 작업 상태 조회 |
| `GET` | `/api/v1/jobs/:id/download` | 완료된 작업의 HWPX 다운로드 |
| `POST` | `/api/v1/validate` | 입력 JSON 검증만 수행 |
| `GET` | `/api/v1/health` | 서버 상태 확인 |

### 동기 변환

요청 후 즉시 HWPX 파일을 응답으로 받습니다.

```bash
curl -X POST http://localhost:8080/api/v1/convert \
  -H "Content-Type: application/json" \
  -d @input.json \
  --output output.hwpx
```

### 비동기 변환

대용량 문서를 비동기로 변환합니다.

```bash
# 1. 변환 요청
curl -X POST http://localhost:8080/api/v1/convert/async \
  -H "Content-Type: application/json" \
  -d @input.json
# 응답: {"job_id":"uuid-here","status":"queued","created_at":"..."}

# 2. 상태 확인
curl http://localhost:8080/api/v1/jobs/{job_id}
# 응답: {"job_id":"...","status":"completed","created_at":"...","completed_at":"..."}

# 3. 결과 다운로드
curl http://localhost:8080/api/v1/jobs/{job_id}/download --output result.hwpx
```

### 검증

변환 없이 입력 JSON의 유효성만 검사합니다.

```bash
curl -X POST http://localhost:8080/api/v1/validate \
  -H "Content-Type: application/json" \
  -d @input.json
# 응답: {"valid":true,"errors":[]}
```

### 상태 확인

```bash
curl http://localhost:8080/api/v1/health
# 응답:
# {
#   "status": "healthy",
#   "version": "0.5.0",
#   "queue": {"pending":0,"processing":0,"completed":0,"failed":0},
#   "workers": {"active":0,"max":4},
#   "uptime_seconds": 120
# }
```

### 에러 응답

모든 에러는 동일한 형식으로 반환됩니다:

```json
{
  "error": {
    "code": "INVALID_JSON",
    "message": "JSON 파싱 실패: expected value at line 1 column 1",
    "details": []
  }
}
```

| 에러 코드 | HTTP 상태 | 설명 |
|-----------|-----------|------|
| `INVALID_JSON` | 400 | JSON 파싱 실패 |
| `INPUT_ERROR` | 400 | 입력 데이터 검증 실패 (article_id 누락 등) |
| `CONVERSION_ERROR` | 500 | 변환 처리 중 오류 |
| `QUEUE_ERROR` | 503 | 작업 큐 제출 실패 |

## Format Support

This library supports HWP 5.0 format files. For older HWP formats, consider using format conversion tools first.

## Writer Features (v0.3.0+)

The HWP writer functionality has been significantly improved with comprehensive feature support:

### ✅ Fully Implemented
- **Hyperlinks**: Complete hyperlink support with proper serialization
  - URL links, email links, file links, bookmarks
  - Multiple hyperlinks per paragraph
  - Custom styling (colors, underline, visited state)
- **Header/Footer**: Full header and footer implementation
  - Custom header/footer text
  - Page numbering with multiple formats (numeric, roman, etc.)
  - Multiple headers/footers per document
- **Page Layout**: Comprehensive page layout control
  - Custom page sizes and standard sizes (A4, Letter, etc.)
  - Portrait/landscape orientation
  - Custom margins (narrow, normal, wide, custom)
  - Multi-column layouts with adjustable spacing
  - Page background colors
- **Tables**: Full table creation and formatting
  - Cell borders and styling
  - Cell merging (horizontal and vertical)
  - Custom cell content
- **Lists/Numbering**: Complete list support
  - Bullet lists with different symbols per level
  - Numbered lists (1., 2., 3., ...)
  - Alphabetic lists (a), b), c), ...)
  - Roman numeral lists (i., ii., iii., ...)
  - Korean lists (가., 나., 다., ...)
  - Nested lists with proper indentation
- **Text Boxes**: Full text box implementation
  - Positioned text boxes
  - Styled text boxes (highlight, warning, info, etc.)
  - Custom styling (borders, backgrounds, alignment)
  - Floating text boxes with rotation and transparency
- **Images**: Complete image insertion
  - PNG, JPEG, BMP, GIF support
  - Custom dimensions and positioning
  - Image captions
  - Proper BinData integration
- **Styled Text**: Rich text formatting
  - Bold, italic, underline, strikethrough
  - Custom fonts and sizes
  - Text colors and background colors
  - Multiple styles in single paragraph
- **Advanced Formatting**:
  - Paragraph alignment (left, center, right, justify)
  - Line spacing control
  - Paragraph spacing (before/after)
  - Headings with automatic sizing
  - Character and paragraph styles
- **Document Properties**: Full metadata support
  - Title, author, subject, keywords
  - Document statistics (character count, word count, etc.)
  - Automatic statistics updates

### ❌ Not Yet Implemented
- **Shapes/Drawing**: Geometric shapes and drawing objects
  - Rectangles, circles, ellipses
  - Lines, arrows, polygons
  - Custom shapes with styling
  - Shapes with text content
  - Shape grouping
  - *(See examples/shape_document.rs.disabled for usage examples)*
- **Charts/Graphs**: Data visualization objects
- **Mathematical Equations**: MathML support
- **Forms**: Input fields and form controls
- **Comments/Annotations**: Review and comment features
- **Track Changes**: Revision history
- **Mail Merge**: Variable field insertion

### 🔧 Known Issues
- No compression support for writer (reader supports both compressed and uncompressed)
- Some advanced table features may have compatibility issues with older Hanword versions

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- HWP file format specification by Hancom Inc.
- Korean text processing community
- Rust parsing and document processing ecosystem
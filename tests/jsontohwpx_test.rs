use std::path::PathBuf;

use hwpers::jsontohwpx::{self, ArticleDocument, ConvertOptions};
use hwpers::HwpxReader;

fn base_path() -> PathBuf {
    PathBuf::from("examples/jsontohwpx")
}

fn default_options() -> ConvertOptions {
    ConvertOptions::default()
}

/// HWPX 바이트가 유효한지 HwpxReader로 검증
fn verify_hwpx_bytes(bytes: &[u8]) -> hwpers::HwpDocument {
    assert!(!bytes.is_empty(), "HWPX 바이트가 비어있음");

    // ZIP 매직 바이트 확인 (PK\x03\x04)
    assert!(
        bytes.len() >= 4 && bytes[0..2] == [0x50, 0x4B],
        "유효한 ZIP 파일이 아닙니다"
    );

    HwpxReader::from_bytes(bytes).expect("HwpxReader가 HWPX 파일을 읽지 못했습니다")
}

/// JSON 문자열 → HWPX 변환 → HwpxReader 검증 헬퍼
fn convert_and_verify(json: &str) -> hwpers::HwpDocument {
    let input: ArticleDocument = serde_json::from_str(json).expect("JSON 파싱 실패");
    let bytes =
        jsontohwpx::convert(&input, &default_options(), &base_path()).expect("변환 실패");
    verify_hwpx_bytes(&bytes)
}

#[test]
fn test_empty_contents_generates_valid_hwpx() {
    let json = r#"{
        "article_id": "EMPTY001",
        "title": "빈 문서",
        "contents": []
    }"#;

    convert_and_verify(json);
}

#[test]
fn test_single_text_generates_valid_hwpx() {
    let json = r#"{
        "article_id": "TEXT001",
        "title": "텍스트 문서",
        "contents": [
            { "type": "text", "value": "안녕하세요, HWPX 문서입니다." }
        ]
    }"#;

    let doc = convert_and_verify(json);
    let text = doc.extract_text();
    assert!(
        text.contains("안녕하세요"),
        "문서에 '안녕하세요' 텍스트가 포함되어야 합니다. 실제: {}",
        text
    );
}

#[test]
fn test_multiline_text() {
    let json = r#"{
        "article_id": "TEXT002",
        "title": "멀티라인",
        "contents": [
            { "type": "text", "value": "첫 줄\n둘째 줄\n셋째 줄" }
        ]
    }"#;

    let doc = convert_and_verify(json);
    let text = doc.extract_text();
    assert!(text.contains("첫 줄"), "첫 줄 포함 확인");
    assert!(text.contains("둘째 줄"), "둘째 줄 포함 확인");
    assert!(text.contains("셋째 줄"), "셋째 줄 포함 확인");
}

#[test]
fn test_multiple_text_contents() {
    let json = r#"{
        "article_id": "TEXT003",
        "title": "연속 텍스트",
        "contents": [
            { "type": "text", "value": "첫 번째 텍스트" },
            { "type": "text", "value": "두 번째 텍스트" }
        ]
    }"#;

    let doc = convert_and_verify(json);
    let text = doc.extract_text();
    assert!(text.contains("첫 번째 텍스트"));
    assert!(text.contains("두 번째 텍스트"));
}

#[test]
fn test_include_header_option() {
    let json = r#"{
        "article_id": "HDR001",
        "title": "헤더 포함 문서",
        "metadata": {
            "author": "홍길동",
            "department": "개발팀"
        },
        "contents": [
            { "type": "text", "value": "본문 내용" }
        ]
    }"#;

    let input: ArticleDocument = serde_json::from_str(json).expect("JSON 파싱 실패");
    let options = ConvertOptions {
        include_header: true,
        header_fields: Vec::new(),
    };
    let bytes = jsontohwpx::convert(&input, &options, &base_path()).expect("변환 실패");
    let doc = verify_hwpx_bytes(&bytes);
    let text = doc.extract_text();
    assert!(text.contains("헤더 포함 문서"), "제목 포함 확인");
    assert!(text.contains("홍길동"), "작성자 포함 확인");
    assert!(text.contains("개발팀"), "부서 포함 확인");
    assert!(text.contains("본문 내용"), "본문 포함 확인");
}

#[test]
fn test_header_fields_filter() {
    let json = r#"{
        "article_id": "HDR002",
        "title": "필터된 헤더",
        "metadata": {
            "author": "숨길 이름"
        },
        "contents": [
            { "type": "text", "value": "본문" }
        ]
    }"#;

    let input: ArticleDocument = serde_json::from_str(json).expect("JSON 파싱 실패");
    let options = ConvertOptions {
        include_header: true,
        header_fields: vec!["title".to_string()],
    };
    let bytes = jsontohwpx::convert(&input, &options, &base_path()).expect("변환 실패");
    let doc = verify_hwpx_bytes(&bytes);
    let text = doc.extract_text();
    assert!(text.contains("필터된 헤더"), "title 포함 확인");
    assert!(!text.contains("숨길 이름"), "author 미포함 확인");
}

#[test]
fn test_table_content() {
    let json = r#"{
        "article_id": "TBL001",
        "title": "테이블 문서",
        "contents": [
            { "type": "table", "value": "<table><tr><td>셀1</td><td>셀2</td></tr><tr><td>셀3</td><td>셀4</td></tr></table>" }
        ]
    }"#;

    convert_and_verify(json);
}

#[test]
fn test_mixed_content_types() {
    let json = r#"{
        "article_id": "MIX001",
        "title": "혼합 문서",
        "contents": [
            { "type": "text", "value": "텍스트 시작" },
            { "type": "table", "value": "<table><tr><td>A</td></tr></table>" },
            { "type": "text", "value": "텍스트 끝" }
        ]
    }"#;

    let doc = convert_and_verify(json);
    let text = doc.extract_text();
    assert!(text.contains("텍스트 시작"));
    assert!(text.contains("텍스트 끝"));
}

#[test]
fn test_convert_to_file() {
    let json = r#"{
        "article_id": "FILE001",
        "title": "파일 저장 테스트",
        "contents": [
            { "type": "text", "value": "파일 테스트" }
        ]
    }"#;

    let input: ArticleDocument = serde_json::from_str(json).expect("JSON 파싱 실패");
    let temp_dir = tempfile::TempDir::new().expect("임시 디렉토리 생성 실패");
    let output_path = temp_dir.path().join("output.hwpx");

    jsontohwpx::convert_to_file(&input, &default_options(), &base_path(), &output_path)
        .expect("파일 저장 실패");

    assert!(output_path.exists(), "출력 파일이 존재해야 함");

    let bytes = std::fs::read(&output_path).expect("파일 읽기 실패");
    verify_hwpx_bytes(&bytes);
}

#[test]
fn test_special_characters_in_text() {
    let json = r#"{
        "article_id": "SPEC001",
        "title": "특수문자 <테스트>",
        "contents": [
            { "type": "text", "value": "특수문자: <tag> & \"quotes\" 'apos'" }
        ]
    }"#;

    convert_and_verify(json);
}

#[test]
fn test_empty_article_id_fails() {
    let json = r#"{
        "article_id": "  ",
        "title": "에러",
        "contents": []
    }"#;

    let input: ArticleDocument = serde_json::from_str(json).expect("JSON 파싱 실패");
    let result = jsontohwpx::convert(&input, &default_options(), &base_path());
    assert!(result.is_err(), "빈 article_id이면 에러를 반환해야 함");
}

// --- 예제 JSON 파일 기반 통합 테스트 ---

/// 예제 JSON 파일을 읽어 변환 + 검증
fn convert_example_file(filename: &str) -> hwpers::HwpDocument {
    let path = base_path().join(filename);
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("예제 파일 읽기 실패: {} ({})", path.display(), e));
    let input: ArticleDocument = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("JSON 파싱 실패: {} ({})", filename, e));
    let bytes = jsontohwpx::convert(&input, &default_options(), &base_path())
        .unwrap_or_else(|e| panic!("변환 실패: {} ({})", filename, e));
    verify_hwpx_bytes(&bytes)
}

#[test]
fn test_example_simple_text() {
    let doc = convert_example_file("simple_text.json");
    let text = doc.extract_text();
    assert!(text.contains("안녕하세요"));
    assert!(text.contains("간단한 텍스트 문서 변환 테스트"));
    assert!(text.contains("세 번째 단락"));
}

#[test]
fn test_example_with_metadata_header() {
    let path = base_path().join("with_metadata_header.json");
    let json = std::fs::read_to_string(&path).unwrap();
    let input: ArticleDocument = serde_json::from_str(&json).unwrap();
    let options = ConvertOptions {
        include_header: true,
        header_fields: Vec::new(),
    };
    let bytes = jsontohwpx::convert(&input, &options, &base_path()).unwrap();
    let doc = verify_hwpx_bytes(&bytes);
    let text = doc.extract_text();
    assert!(text.contains("사내 시스템 점검 안내"), "제목 포함");
    assert!(text.contains("2025년 2월 1일"), "본문 내용 포함");
}

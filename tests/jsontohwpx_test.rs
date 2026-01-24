use std::path::PathBuf;

use hwpers::jsontohwpx::{self, ApiResponse};
use hwpers::HwpxReader;

fn base_path() -> PathBuf {
    PathBuf::from("examples/jsontohwpx")
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
    let input: ApiResponse = serde_json::from_str(json).expect("JSON 파싱 실패");
    let bytes = jsontohwpx::convert(&input, &base_path()).expect("변환 실패");
    verify_hwpx_bytes(&bytes)
}

#[test]
fn test_empty_contents_generates_valid_hwpx() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "EMPTY001",
                "subject": "빈 문서",
                "contents": []
            }
        }
    }"#;

    convert_and_verify(json);
}

#[test]
fn test_single_text_generates_valid_hwpx() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TEXT001",
                "subject": "텍스트 문서",
                "contents": [
                    { "type": "text", "value": "안녕하세요, HWPX 문서입니다." }
                ]
            }
        }
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
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TEXT002",
                "subject": "멀티라인",
                "contents": [
                    { "type": "text", "value": "첫 줄\n둘째 줄\n셋째 줄" }
                ]
            }
        }
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
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TEXT003",
                "subject": "연속 텍스트",
                "contents": [
                    { "type": "text", "value": "첫 번째 텍스트" },
                    { "type": "text", "value": "두 번째 텍스트" }
                ]
            }
        }
    }"#;

    let doc = convert_and_verify(json);
    let text = doc.extract_text();
    assert!(text.contains("첫 번째 텍스트"));
    assert!(text.contains("두 번째 텍스트"));
}

#[test]
fn test_include_header_option() {
    let json = r#"{
        "responseCode": "0",
        "options": {
            "includeHeader": true
        },
        "data": {
            "article": {
                "atclId": "HDR001",
                "subject": "헤더 포함 문서",
                "regEmpName": "홍길동",
                "regDeptName": "개발팀",
                "contents": [
                    { "type": "text", "value": "본문 내용" }
                ]
            }
        }
    }"#;

    let doc = convert_and_verify(json);
    let text = doc.extract_text();
    assert!(text.contains("헤더 포함 문서"), "제목 포함 확인");
    assert!(text.contains("홍길동"), "작성자 포함 확인");
    assert!(text.contains("개발팀"), "부서 포함 확인");
    assert!(text.contains("본문 내용"), "본문 포함 확인");
}

#[test]
fn test_header_fields_filter() {
    let json = r#"{
        "responseCode": "0",
        "options": {
            "includeHeader": true,
            "headerFields": ["subject"]
        },
        "data": {
            "article": {
                "atclId": "HDR002",
                "subject": "필터된 헤더",
                "regEmpName": "숨길 이름",
                "contents": [
                    { "type": "text", "value": "본문" }
                ]
            }
        }
    }"#;

    let doc = convert_and_verify(json);
    let text = doc.extract_text();
    assert!(text.contains("필터된 헤더"), "subject 포함 확인");
    assert!(!text.contains("숨길 이름"), "regEmpName 미포함 확인");
}

#[test]
fn test_table_content() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL001",
                "subject": "테이블 문서",
                "contents": [
                    { "type": "table", "value": "<table><tr><td>셀1</td><td>셀2</td></tr><tr><td>셀3</td><td>셀4</td></tr></table>" }
                ]
            }
        }
    }"#;

    convert_and_verify(json);
}

#[test]
fn test_mixed_content_types() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "MIX001",
                "subject": "혼합 문서",
                "contents": [
                    { "type": "text", "value": "텍스트 시작" },
                    { "type": "table", "value": "<table><tr><td>A</td></tr></table>" },
                    { "type": "text", "value": "텍스트 끝" }
                ]
            }
        }
    }"#;

    let doc = convert_and_verify(json);
    let text = doc.extract_text();
    assert!(text.contains("텍스트 시작"));
    assert!(text.contains("텍스트 끝"));
}

#[test]
fn test_convert_to_file() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "FILE001",
                "subject": "파일 저장 테스트",
                "contents": [
                    { "type": "text", "value": "파일 테스트" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).expect("JSON 파싱 실패");
    let temp_dir = tempfile::TempDir::new().expect("임시 디렉토리 생성 실패");
    let output_path = temp_dir.path().join("output.hwpx");

    jsontohwpx::convert_to_file(&input, &base_path(), &output_path).expect("파일 저장 실패");

    assert!(output_path.exists(), "출력 파일이 존재해야 함");

    let bytes = std::fs::read(&output_path).expect("파일 읽기 실패");
    verify_hwpx_bytes(&bytes);
}

#[test]
fn test_special_characters_in_text() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "SPEC001",
                "subject": "특수문자 <테스트>",
                "contents": [
                    { "type": "text", "value": "특수문자: <tag> & \"quotes\" 'apos'" }
                ]
            }
        }
    }"#;

    convert_and_verify(json);
}

#[test]
fn test_invalid_response_code_fails() {
    let json = r#"{
        "responseCode": "999",
        "responseText": "FAIL",
        "data": {
            "article": {
                "atclId": "ERR001",
                "subject": "에러",
                "contents": []
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).expect("JSON 파싱 실패");
    let result = jsontohwpx::convert(&input, &base_path());
    assert!(result.is_err(), "responseCode != 0이면 에러를 반환해야 함");
}

// --- 예제 JSON 파일 기반 통합 테스트 ---

/// 예제 JSON 파일을 읽어 변환 + 검증
fn convert_example_file(filename: &str) -> hwpers::HwpDocument {
    let path = base_path().join(filename);
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("예제 파일 읽기 실패: {} ({})", path.display(), e));
    let input: ApiResponse = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("JSON 파싱 실패: {} ({})", filename, e));
    let bytes = jsontohwpx::convert(&input, &base_path())
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
    let doc = convert_example_file("with_metadata_header.json");
    let text = doc.extract_text();
    assert!(text.contains("사내 시스템 점검 안내"), "제목 포함");
    assert!(text.contains("김관리"), "작성자 포함");
    assert!(text.contains("IT인프라팀"), "부서 포함");
    assert!(text.contains("2025년 2월 1일"), "본문 내용 포함");
}

use std::path::PathBuf;

use hwpers::jsontohwpx::{self, ApiResponse};
use hwpers::HwpxReader;

fn base_path() -> PathBuf {
    PathBuf::from("examples/jsontohwpx")
}

fn verify_hwpx_bytes(bytes: &[u8]) {
    assert!(!bytes.is_empty());
    assert!(bytes[0..2] == [0x50, 0x4B], "유효한 ZIP 파일이 아닙니다");
    HwpxReader::from_bytes(bytes).expect("HwpxReader가 HWPX 파일을 읽지 못했습니다");
}

fn convert_example_file(filename: &str) -> hwpers::HwpDocument {
    let path = base_path().join(filename);
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("예제 파일 읽기 실패: {} ({})", path.display(), e));
    let input: ApiResponse = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("JSON 파싱 실패: {} ({})", filename, e));
    let bytes = jsontohwpx::convert(&input, &base_path())
        .unwrap_or_else(|e| panic!("변환 실패: {} ({})", filename, e));
    verify_hwpx_bytes(&bytes);
    HwpxReader::from_bytes(&bytes).unwrap()
}

#[test]
fn test_with_table_example() {
    let doc = convert_example_file("with_table.json");
    let text = doc.extract_text();
    // 테이블 외 텍스트 확인 (HwpxReader의 extract_text()는 테이블 셀 미포함)
    assert!(text.contains("1분기 실적"), "본문 텍스트 포함");
    assert!(text.contains("15% 성장"), "후행 텍스트 포함");
}

#[test]
fn test_table_merge_example() {
    // colspan/rowspan 포함 테이블: 병합은 무시하고 텍스트만 추출
    let doc = convert_example_file("table_merge.json");
    let text = doc.extract_text();
    // 테이블 외 텍스트 확인
    assert!(text.contains("셀 병합이 포함된"), "본문 텍스트 포함");
}

#[test]
fn test_table_with_text_before_after() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL_MIX001",
                "subject": "혼합",
                "contents": [
                    { "type": "text", "value": "표 앞 텍스트" },
                    { "type": "table", "value": "<table><tr><td>셀</td></tr></table>" },
                    { "type": "text", "value": "표 뒤 텍스트" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let doc = HwpxReader::from_bytes(&bytes).unwrap();
    let text = doc.extract_text();
    assert!(text.contains("표 앞 텍스트"));
    assert!(text.contains("표 뒤 텍스트"));
}

#[test]
fn test_multiple_tables() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL_MULTI001",
                "subject": "다중 테이블",
                "contents": [
                    { "type": "table", "value": "<table><tr><td>첫째 표</td></tr></table>" },
                    { "type": "text", "value": "중간 텍스트" },
                    { "type": "table", "value": "<table><tr><td>둘째 표</td></tr></table>" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);
}

#[test]
fn test_invalid_html_table() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL_ERR001",
                "subject": "에러",
                "contents": [
                    { "type": "table", "value": "<table></table>" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).unwrap();
    let result = jsontohwpx::convert(&input, &base_path());
    assert!(result.is_err(), "빈 테이블은 에러를 반환해야 함");
}

#[test]
fn test_large_table() {
    let mut rows_html = String::new();
    for i in 0..20 {
        rows_html.push_str(&format!(
            "<tr><td>행{}</td><td>값A{}</td><td>값B{}</td><td>값C{}</td></tr>",
            i, i, i, i
        ));
    }
    let html = format!(
        "<table><thead><tr><th>번호</th><th>A</th><th>B</th><th>C</th></tr></thead><tbody>{}</tbody></table>",
        rows_html
    );

    let json = format!(
        r#"{{"responseCode":"0","data":{{"article":{{"atclId":"TBL_BIG001","subject":"큰 테이블","contents":[{{"type":"table","value":"{}"}}]}}}}}}"#,
        html.replace('"', "\\\"")
    );

    let input: ApiResponse = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);
}
